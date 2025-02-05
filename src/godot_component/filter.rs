use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult, Write as _};
use std::ops::{Bound, Range, RangeBounds};

use godot::prelude::*;
use nom::bytes::complete::take_while1;
use nom::character::complete::{alpha1, char as char_, space0, space1};
use nom::combinator::{all_consuming, opt, value};
use nom::error::{ErrorKind, ParseError};
use nom::sequence::preceded;
use nom::{Err as NomErr, IResult, Parser};
use rbitset::BitSet;

use crate::godot_util::to_lower_inline_smol_str;
use crate::rw_struct::{CharSlice, SingleError};

#[derive(Debug, Clone)]
pub struct FilterFlags<const N: usize>(BitSet<u8, N>);

#[derive(Debug, Clone, Copy)]
pub struct FilterFlagsRef<'a, const N: usize> {
    r: &'a FilterFlags<N>,
    o: usize,
    l: usize,
}

#[derive(Debug)]
pub struct FilterFlagsMut<'a, const N: usize> {
    r: &'a mut FilterFlags<N>,
    o: usize,
    l: usize,
}

impl<const N: usize> Default for FilterFlags<N> {
    fn default() -> Self {
        let mut ret = <BitSet<u8, N>>::new();
        ret.fill(0..N, true);
        Self(ret)
    }
}

fn rebound(o: usize, l: usize, bs: Bound<&usize>, be: Bound<&usize>) -> Option<(usize, usize)> {
    let a = match bs {
        Bound::Unbounded => 0,
        Bound::Included(&v) => v,
        Bound::Excluded(&v) => v.checked_add(1)?,
    };
    let b = match be {
        Bound::Unbounded => l,
        Bound::Included(&v) => v.checked_add(1)?,
        Bound::Excluded(&v) => v,
    };

    if a > b || b > l {
        None
    } else if a == b {
        Some((0, 0))
    } else {
        Some((o + a, b - a))
    }
}

#[allow(dead_code)]
impl<const N: usize> FilterFlags<N> {
    #[inline]
    pub fn as_ref(&self) -> FilterFlagsRef<'_, N> {
        FilterFlagsRef {
            r: self,
            o: 0,
            l: N * 8,
        }
    }

    #[inline]
    pub fn as_mut(&mut self) -> FilterFlagsMut<'_, N> {
        FilterFlagsMut {
            r: self,
            o: 0,
            l: N * 8,
        }
    }

    #[inline]
    pub fn slice(&self, i: impl RangeBounds<usize> + Debug) -> FilterFlagsRef<'_, N> {
        self.as_ref().slice(i)
    }

    #[inline]
    pub fn slice_mut(&mut self, i: impl RangeBounds<usize> + Debug) -> FilterFlagsMut<'_, N> {
        self.as_mut().into_slice_mut(i)
    }

    pub fn get(&self, i: usize) -> bool {
        self.0.try_contains(i).unwrap_or_default()
    }

    pub fn set(&mut self, i: usize, v: bool) {
        let _ = if v {
            self.0.try_insert(i)
        } else {
            self.0.try_remove(i)
        };
    }

    pub fn fill(&mut self, Range { start, end }: Range<usize>, v: bool) {
        self.0.fill(start.min(N * 8)..end.min(N * 8), v);
    }
}

#[allow(dead_code)]
impl<const N: usize> FilterFlagsRef<'_, N> {
    pub fn slice(&self, i: impl RangeBounds<usize> + Debug) -> Self {
        let Self { r, o, l } = *self;
        let Some((o, l)) = rebound(o, l, i.start_bound(), i.end_bound()) else {
            panic!("Index {i:?} out of bounds (length: {l})")
        };
        Self { r, o, l }
    }

    pub fn get(&self, i: usize) -> bool {
        match i.checked_add(self.o) {
            Some(v) if i < self.l => self.r.get(v),
            _ => false,
        }
    }
}

#[allow(dead_code)]
impl<'a, const N: usize> FilterFlagsMut<'a, N> {
    pub fn slice(&self, i: impl RangeBounds<usize> + Debug) -> FilterFlagsRef<'_, N> {
        let Some((o, l)) = rebound(self.o, self.l, i.start_bound(), i.end_bound()) else {
            panic!("Index {:?} out of bounds (length: {})", i, self.l)
        };
        FilterFlagsRef { r: &*self.r, o, l }
    }

    pub fn into_slice(self, i: impl RangeBounds<usize> + Debug) -> FilterFlagsRef<'a, N> {
        let Some((o, l)) = rebound(self.o, self.l, i.start_bound(), i.end_bound()) else {
            panic!("Index {:?} out of bounds (length: {})", i, self.l)
        };
        FilterFlagsRef { r: &*self.r, o, l }
    }

    pub fn slice_mut(&mut self, i: impl RangeBounds<usize> + Debug) -> FilterFlagsMut<'_, N> {
        let r = &mut *self.r;
        let Some((o, l)) = rebound(self.o, self.l, i.start_bound(), i.end_bound()) else {
            panic!("Index {:?} out of bounds (length: {})", i, self.l)
        };
        FilterFlagsMut { r, o, l }
    }

    pub fn into_slice_mut(self, i: impl RangeBounds<usize> + Debug) -> Self {
        let Some((o, l)) = rebound(self.o, self.l, i.start_bound(), i.end_bound()) else {
            panic!("Index {:?} out of bounds (length: {})", i, self.l)
        };
        FilterFlagsMut { r: self.r, o, l }
    }

    pub fn get(&self, i: usize) -> bool {
        match i.checked_add(self.o) {
            Some(v) if i < self.l => self.r.get(v),
            _ => false,
        }
    }

    pub fn set(&mut self, i: usize, v: bool) {
        match i.checked_add(self.o) {
            Some(t) if i < self.l => self.r.set(t, v),
            _ => (),
        }
    }

    pub fn fill(&mut self, i: Range<usize>, v: bool) {
        let s = match i.start.checked_add(self.o) {
            Some(t) if i.start < self.l => t,
            _ => return,
        };
        let e = match i.end.checked_add(self.o) {
            Some(t) if i.end < self.l => t,
            _ => self.o + self.l,
        };
        self.r.fill(s..e, v);
    }

    pub fn fill_all(&mut self, v: bool) {
        self.r.fill(self.o..self.o + self.l, v);
    }
}

impl<'a, const N: usize> From<&'a FilterFlags<N>> for FilterFlagsRef<'a, N> {
    fn from(v: &'a FilterFlags<N>) -> Self {
        v.as_ref()
    }
}

impl<'a, const N: usize> From<&'a mut FilterFlags<N>> for FilterFlagsMut<'a, N> {
    fn from(v: &'a mut FilterFlags<N>) -> Self {
        v.as_mut()
    }
}

#[macro_export]
macro_rules! filter_macro {
    (#c) => {
        pub const filter_len: usize = 0;
    };
    (#c <$s:ident>) => {
        pub const filter_len: usize = $s + 1;
    };
    (#c $(<$s:ident>)? $i0:ident) => {
        pub const $i0: usize = $($s + 1 +)? 0;
        pub const filter_len: usize = $i0 + 1;
    };
    (#c $(<$s:ident>)? $i0:ident, $i1:ident) => {
        pub const $i0: usize = $($s + 1 +)? 0;
        pub const $i1: usize = $($s + 1 +)? 1;
        pub const filter_len: usize = $i1 + 1;
    };
    (#c $(<$s:ident>)? $i0:ident, $i1:ident, $i2:ident) => {
        pub const $i0: usize = $($s + 1 +)? 0;
        pub const $i1: usize = $($s + 1 +)? 1;
        pub const $i2: usize = $($s + 1 +)? 2;
        pub const filter_len: usize = $i2 + 1;
    };
    (#c $(<$s:ident>)? $i0:ident, $i1:ident, $i2:ident, $i3:ident) => {
        pub const $i0: usize = $($s + 1 +)? 0;
        pub const $i1: usize = $($s + 1 +)? 1;
        pub const $i2: usize = $($s + 1 +)? 2;
        pub const $i3: usize = $($s + 1 +)? 3;
        pub const filter_len: usize = $i3 + 1;
    };
    (#c $(<$s:ident>)? $i0:ident, $i1:ident, $i2:ident, $i3:ident, $i4:ident $(, $i:ident)*) => {
        pub const $i0: usize = $($s + 1 +)? 0;
        pub const $i1: usize = $($s + 1 +)? 1;
        pub const $i2: usize = $($s + 1 +)? 2;
        pub const $i3: usize = $($s + 1 +)? 3;
        pub const $i4: usize = $($s + 1 +)? 4;
        $crate::filter_macro!{#c <$i4> $($i),*}
    };
    (#cp) => {
        pub const filter_len: usize = 0;
    };
    (#cp <$s:ident>) => {
        pub const filter_len: usize = $s.0 + $s.1;
    };
    (#cp $(<$s:ident>)? $i0:ident) => {
        pub const $i0: (usize, usize) = ($($s.0 + $s.1 +)? 0, super::$i0::indices::filter_len);
        pub const filter_len: usize = $i0.0 + $i0.1;
    };
    (#cp $(<$s:ident>)? $i0:ident, $i1:ident) => {
        pub const $i0: (usize, usize) = ($($s.0 + $s.1 +)? 0, super::$i0::indices::filter_len);
        pub const $i1: (usize, usize) = ($i0.0 + $i0.1, super::$i1::indices::filter_len);
        pub const filter_len: usize = $i1.0 + $i1.1;
    };
    (#cp $(<$s:ident>)? $i0:ident, $i1:ident, $i2:ident) => {
        pub const $i0: (usize, usize) = ($($s.0 + $s.1 +)? 0, super::$i0::indices::filter_len);
        pub const $i1: (usize, usize) = ($i0.0 + $i0.1, super::$i1::indices::filter_len);
        pub const $i2: (usize, usize) = ($i1.0 + $i1.1, super::$i2::indices::filter_len);
        pub const filter_len: usize = $i2.0 + $i2.1;
    };
    (#cp $(<$s:ident>)? $i0:ident, $i1:ident, $i2:ident, $i3:ident) => {
        pub const $i0: (usize, usize) = ($($s.0 + $s.1 +)? 0, super::$i0::indices::filter_len);
        pub const $i1: (usize, usize) = ($i0.0 + $i0.1, super::$i1::indices::filter_len);
        pub const $i2: (usize, usize) = ($i1.0 + $i1.1, super::$i2::indices::filter_len);
        pub const $i3: (usize, usize) = ($i2.0 + $i2.1, super::$i3::indices::filter_len);
        pub const filter_len: usize = $i3.0 + $i3.1;
    };
    (#cp $(<$s:ident>)? $i0:ident, $i1:ident, $i2:ident, $i3:ident, $i4:ident $(, $i:ident)*) => {
        pub const $i0: (usize, usize) = ($($s.0 + $s.1 +)? 0, super::$i0::indices::filter_len);
        pub const $i1: (usize, usize) = ($i0.0 + $i0.1, super::$i1::indices::filter_len);
        pub const $i2: (usize, usize) = ($i1.0 + $i1.1, super::$i2::indices::filter_len);
        pub const $i3: (usize, usize) = ($i2.0 + $i2.1, super::$i3::indices::filter_len);
        pub const $i4: (usize, usize) = ($i3.0 + $i3.1, super::$i4::indices::filter_len);
        $crate::filter_macro!{#cp <$i4> $($i),*}
    };
    (filter $e:expr, $module:ident, $interface:ident, $method:ident) => {
        $crate::site_context!($crate::godot_component::filter_data::run_filter(
            $e,
            $crate::godot_component::filter_data::indices::$module.0 +
            $crate::godot_component::filter_data::$module::indices::$interface.0 +
            $crate::godot_component::filter_data::$module::$interface::indices::$method,
        ))
    };
    ($t:ident [$($i:ident -> $s:literal),* $(,)?]) => {
        pub mod filter_data {
            #[allow(non_upper_case_globals)]
            pub mod indices {
                $crate::filter_macro!{#c $($i),*}
            }

            pub fn parse_filter<const N: usize>(mut filter: $crate::godot_component::filter::FilterFlagsMut<'_, N>, item: $crate::godot_component::filter::FilterItem<'_>) {
                match item.$t {
                    None => filter.fill_all(item.allow),
                    $(Some($s) => filter.set(indices::$i, item.allow),)*
                    _ => (),
                }
            }

            pub fn run_filter<const N: usize>(filter: $crate::godot_component::filter::FilterFlagsRef<'_, N>, i: usize) -> Result<(), $crate::godot_component::filter::FilterItem<'static>> {
                if filter.get(i) {
                    Ok(())
                }
                $(else if i == indices::$i {
                    Err($crate::godot_component::filter::FilterItem {
                        $t: Some($s),
                        ..$crate::godot_component::filter::FilterItem::default()
                    })
                })*
                else {
                    Err($crate::godot_component::filter::FilterItem::default())
                }
            }

            #[cfg(test)]
            pub fn print_filter<const N: usize>(filter: $crate::godot_component::filter::FilterFlagsRef<'_, N>, f: $crate::godot_component::filter::FilterItem<'_>) {
                $(($crate::godot_component::filter::FilterItem {
                    allow: filter.get(indices::$i),
                    $t: Some($s),
                    ..f
                }).print_filter();)*
            }
        }
    };
    ($t:ident [$($i:ident <$($p:ident)::+> -> $s:literal),* $(,)?]) => {
        pub mod filter_data {
            $(pub use super::$($p::)+filter_data as $i;)*
            #[allow(non_upper_case_globals)]
            pub mod indices {
                $crate::filter_macro!{#cp $($i),*}
            }

            pub fn parse_filter<const N: usize>(mut filter: $crate::godot_component::filter::FilterFlagsMut<'_, N>, item: $crate::godot_component::filter::FilterItem<'_>) {
                $(if let None | Some($s) = item.$t {
                    $i::parse_filter(filter.slice_mut(indices::$i.0..indices::$i.0 + indices::$i.1), item)
                })*
            }

            pub fn run_filter<const N: usize>(filter: $crate::godot_component::filter::FilterFlagsRef<'_, N>, i: usize) -> Result<(), $crate::godot_component::filter::FilterItem<'static>> {
                $(if i < indices::$i.0 + indices::$i.1 {
                    match $i::run_filter(filter.slice(indices::$i.0..indices::$i.0 + indices::$i.1), i) {
                        Err(e) => Err($crate::godot_component::filter::FilterItem {
                            $t: Some($s),
                            ..e
                        }),
                        v => v,
                    }
                } else)*
                {
                    Err($crate::godot_component::filter::FilterItem::default())
                }
            }

            #[cfg(test)]
            pub fn print_filter<const N: usize>(filter: $crate::godot_component::filter::FilterFlagsRef<'_, N>, f: $crate::godot_component::filter::FilterItem<'_>) {
                $($i::print_filter(
                    filter.slice(indices::$i.0..indices::$i.0 + indices::$i.1),
                    $crate::godot_component::filter::FilterItem {
                        $t: Some($s),
                        ..f
                    },
                );)*
            }
        }
    };
}

use crate::godot_component::filter_data::indices::filter_len as ENDPOINT;
use crate::godot_component::filter_data::parse_filter;
#[cfg(test)]
use crate::godot_component::filter_data::print_filter;
const DATA_LEN: usize = (ENDPOINT + 7) / 8;

pub type Filter = FilterFlags<DATA_LEN>;

impl GodotConvert for Filter {
    type Via = Dictionary;
}

impl FromGodot for Filter {
    fn try_from_variant(v: &Variant) -> Result<Self, ConvertError> {
        if v.is_nil() {
            Ok(Self::default())
        } else if v.get_type() == VariantType::STRING {
            let v = v.try_to::<GString>()?;
            parse_script(CharSlice(v.chars())).map_err(|e| ConvertError::with_error_value(e, v))
        } else {
            from_dict(v.try_to()?)
        }
    }

    fn try_from_godot(via: Self::Via) -> Result<Self, ConvertError> {
        from_dict(via)
    }

    fn from_godot(via: Self::Via) -> Self {
        from_dict(via).unwrap()
    }
}

fn from_dict(d: Dictionary) -> Result<Filter, ConvertError> {
    let f = |s: &mut String, k: Variant| -> Result<(), ConvertError> {
        s.clear();
        if k.get_type() == VariantType::STRING_NAME {
            write!(s, "{}", k.to::<StringName>())
        } else {
            write!(s, "{}", k.try_to::<GString>()?)
        }
        .map_err(|e| ConvertError::with_error_value(e, k))
    };
    let mut ret = Filter::default();
    let mut fi = ret.slice_mut(..ENDPOINT);
    let mut module = String::new();
    let mut interface = String::new();
    let mut method = String::new();
    for (k, v) in d.iter_shared() {
        f(&mut module, k)?;
        if module == "*" {
            parse_filter(
                fi.slice_mut(..),
                FilterItem {
                    allow: v.try_to()?,
                    ..FilterItem::default()
                },
            );
            continue;
        }

        if v.get_type() == VariantType::BOOL {
            parse_filter(
                fi.slice_mut(..),
                FilterItem {
                    allow: v.to(),
                    module: Some(&module),
                    ..FilterItem::default()
                },
            );
            continue;
        }

        for (k, v) in v.try_to::<Dictionary>()?.iter_shared() {
            f(&mut interface, k)?;
            if interface == "*" {
                parse_filter(
                    fi.slice_mut(..),
                    FilterItem {
                        allow: v.try_to()?,
                        module: Some(&module),
                        ..FilterItem::default()
                    },
                );
                continue;
            }

            if v.get_type() == VariantType::BOOL {
                parse_filter(
                    fi.slice_mut(..),
                    FilterItem {
                        allow: v.to(),
                        module: Some(&module),
                        interface: Some(&interface),
                        ..FilterItem::default()
                    },
                );
                continue;
            }

            for (k, v) in v.try_to::<Dictionary>()?.iter_shared() {
                f(&mut method, k)?;
                let allow = v.try_to::<bool>()?;
                if method == "*" {
                    parse_filter(
                        fi.slice_mut(..),
                        FilterItem {
                            allow,
                            module: Some(&module),
                            interface: Some(&interface),
                            ..FilterItem::default()
                        },
                    );
                    continue;
                }

                parse_filter(
                    fi.slice_mut(..),
                    FilterItem {
                        allow,
                        module: Some(&module),
                        interface: Some(&interface),
                        method: Some(&method),
                    },
                );
            }
        }
    }

    Ok(ret)
}

#[derive(Default, Clone, Copy)]
pub struct FilterItem<'a> {
    pub allow: bool,
    pub module: Option<&'a str>,
    pub interface: Option<&'a str>,
    pub method: Option<&'a str>,
}

impl FilterItem<'_> {
    #[cfg(test)]
    pub fn print_filter(&self) {
        eprintln!(
            "{} {}.{}.{}",
            if self.allow { "A" } else { "D" },
            self.module.unwrap_or("*"),
            self.interface.unwrap_or("*"),
            self.method.unwrap_or("*"),
        );
    }
}

impl Debug for FilterItem<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        static UNKNOWN: &str = "<unknown>";
        write!(
            f,
            "Calling {}.{}.{} is blocked!",
            self.module.unwrap_or(UNKNOWN),
            self.interface.unwrap_or(UNKNOWN),
            self.method.unwrap_or(UNKNOWN)
        )
    }
}

impl Display for FilterItem<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <Self as Debug>::fmt(self, f)
    }
}

impl Error for FilterItem<'_> {}

#[allow(clippy::type_complexity)]
fn parse_line(
    i: CharSlice<'_>,
) -> IResult<CharSlice<'_>, Option<(bool, [Option<&'_ [char]>; 3])>, SingleError<CharSlice<'_>>> {
    fn item_or_any(
        i: CharSlice<'_>,
    ) -> IResult<CharSlice<'_>, Option<CharSlice<'_>>, SingleError<CharSlice<'_>>> {
        take_while1(|c: char| c.is_alphanumeric() || matches!(c, ':' | '-'))
            .map(Some)
            .or(value(None, char_('*')))
            .parse_complete(i)
    }

    fn maybe_item_or_any(
        i: CharSlice<'_>,
    ) -> IResult<CharSlice<'_>, Option<&'_ [char]>, SingleError<CharSlice<'_>>> {
        opt(item_or_any)
            .map(|v| v.flatten().map(|v| v.0))
            .parse_complete(i)
    }

    fn maybe_dotted_item_or_any(
        i: CharSlice<'_>,
    ) -> IResult<CharSlice<'_>, Option<&'_ [char]>, SingleError<CharSlice<'_>>> {
        opt(preceded(char_('.'), item_or_any))
            .map(|v| v.flatten().map(|v| v.0))
            .parse_complete(i)
    }

    let (i, _) = space0(i)?;
    if matches!(i.0, [] | ['#', ..] | ['/', '/', ..]) {
        // Comment
        return Ok((CharSlice(&[]), None));
    }

    let (i, v) = alpha1(i)?;
    let allow = match to_lower_inline_smol_str(v.0).as_deref() {
        Some("deny" | "d") => false,
        Some("allow" | "a") => true,
        _ => {
            return Err(NomErr::Error(SingleError::from_error_kind(
                v,
                ErrorKind::OneOf,
            )))
        }
    };
    let (i, _) = space1(i)?;

    let (i, module) = maybe_item_or_any(i)?;
    let (i, interface) = if module.is_some() {
        maybe_dotted_item_or_any(i)?
    } else {
        (i, None)
    };
    let (i, method) = if interface.is_some() {
        maybe_dotted_item_or_any(i)?
    } else {
        (i, None)
    };
    let (i, _) = all_consuming(space0).parse_complete(i)?;

    Ok((i, Some((allow, [module, interface, method]))))
}

fn parse_script(s: CharSlice<'_>) -> Result<Filter, NomErr<SingleError<String>>> {
    let mut ret = Filter::default();
    let mut f = ret.slice_mut(..ENDPOINT);
    let mut module = String::new();
    let mut interface = String::new();
    let mut method = String::new();
    for s in s.0.split(|c| *c == '\n').map(CharSlice) {
        let (_, v) = parse_line(s).map_err(|e| e.map(SingleError::into_owned))?;

        let Some((allow, t)) = v else {
            continue;
        };
        let module = if let Some(t) = t[0] {
            module.clear();
            module.extend(t);
            Some(&*module)
        } else {
            None
        };
        let interface = if let Some(t) = t[1] {
            interface.clear();
            interface.extend(t);
            Some(&*interface)
        } else {
            None
        };
        let method = if let Some(t) = t[2] {
            method.clear();
            method.extend(t);
            Some(&*method)
        } else {
            None
        };
        parse_filter(
            f.slice_mut(..),
            FilterItem {
                module,
                interface,
                method,
                allow,
            },
        );
    }

    Ok(ret)
}

#[cfg(test)]
mod tests {
    use super::*;

    use proptest::collection::vec;
    use proptest::prelude::*;

    fn to_char_array(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    #[test]
    fn test_filter_flags() {
        const SCRIPT: &str = r"
# How filter script works:
# Start applying from top to bottom
# Lower filter override higher filter
// Use # or // to comment
deny *
allow godot:core.typeis
allow godot:core.primitive.*
    deny godot:core.primitive.from-vector2i
deny godot:core.primitive.to-vector2i";
        let f = parse_script(CharSlice(&to_char_array(SCRIPT))).unwrap();
        println!("{:?}", f);
        print_filter(f.as_ref(), FilterItem::default());
    }

    #[test]
    fn test_filter_script() {
        fn f(v: Vec<String>) {
            let s = v
                .iter()
                .flat_map(|s| s.chars().chain(['\n']))
                .collect::<Vec<char>>();
            drop(v);
            parse_script(CharSlice(&s)).unwrap();
        }

        const REGEX_LINE: &str = "[ \\t]*((//|#)[^\\n]*|(allow|a|deny|d)[ \\t]+(\\*|godot:core(\\.(\\*|primitive(\\.(\\*|to-vector2i))?))?))?";
        proptest!(|(v in vec(REGEX_LINE, 0..32))| f(v));
    }
}

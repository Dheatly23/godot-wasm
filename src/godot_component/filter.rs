use std::error::Error;
use std::fmt::{Debug, Display, Formatter, Result as FmtResult, Write as _};
use std::ops::{Bound, Range, RangeBounds};

use godot::builtin::meta::{ConvertError, GodotConvert};
use godot::prelude::*;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::{char as char_, space0};
use nom::combinator::{map, opt};
use nom::sequence::{delimited, pair, preceded, separated_pair};
use nom::{Err as NomErr, IResult};

use crate::rw_struct::{CharSlice, SingleError};

#[derive(Debug, Clone)]
pub struct FilterFlags<const N: usize>([u8; N]);

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
        Self([255; N])
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
    pub fn as_ref<'a>(&'a self) -> FilterFlagsRef<'a, N> {
        FilterFlagsRef {
            r: self,
            o: 0,
            l: N * 8,
        }
    }

    #[inline]
    pub fn as_mut<'a>(&'a mut self) -> FilterFlagsMut<'a, N> {
        FilterFlagsMut {
            r: self,
            o: 0,
            l: N * 8,
        }
    }

    #[inline]
    pub fn slice<'a>(&'a self, i: impl RangeBounds<usize> + Debug) -> FilterFlagsRef<'a, N> {
        self.as_ref().slice(i)
    }

    #[inline]
    pub fn slice_mut<'a>(
        &'a mut self,
        i: impl RangeBounds<usize> + Debug,
    ) -> FilterFlagsMut<'a, N> {
        self.as_mut().into_slice_mut(i)
    }

    pub fn get(&self, i: usize) -> bool {
        let (i, r) = (i / 8, i % 8);
        match self.0.get(i) {
            Some(&v) => v & (1 << r) != 0,
            None => false,
        }
    }

    pub fn set(&mut self, i: usize, v: bool) {
        let (i, r) = (i / 8, i % 8);
        if let Some(p) = self.0.get_mut(i) {
            if v {
                *p |= 1 << r;
            } else {
                *p &= !(1 << r);
            }
        }
    }

    pub fn fill(&mut self, Range { start, end }: Range<usize>, v: bool) {
        let (is, rs) = (start / 8, start % 8);
        let (ie, re) = (end / 8, end % 8);
        for i in is..=ie {
            let Some(p) = self.0.get_mut(i) else { continue };
            if i == is {
                if v {
                    *p |= 255 << rs;
                } else {
                    *p &= !(255 << rs);
                }
            } else if i == ie {
                if v {
                    *p |= !(255 << re);
                } else {
                    *p &= 255 << rs;
                }
            } else {
                *p = if v { 255 } else { 0 };
            }
        }
    }
}

#[allow(dead_code)]
impl<'a, const N: usize> FilterFlagsRef<'a, N> {
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
    pub fn slice<'b>(&'b self, i: impl RangeBounds<usize> + Debug) -> FilterFlagsRef<'b, N> {
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

    pub fn slice_mut<'b>(
        &'b mut self,
        i: impl RangeBounds<usize> + Debug,
    ) -> FilterFlagsMut<'b, N> {
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
                match item.$t {
                    None => filter.fill_all(item.allow),
                    $(Some($s) => $i::parse_filter(filter.into_slice_mut(indices::$i.0..indices::$i.0 + indices::$i.1), item),)*
                    _ => (),
                }
            }

            pub fn run_filter<const N: usize>(filter: $crate::godot_component::filter::FilterFlagsRef<'_, N>, i: usize) -> Result<(), $crate::godot_component::filter::FilterItem<'static>> {
                $(if i < indices::$i.0 + indices::$i.1 {
                    match $i::run_filter(filter, i) {
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
        }
    };
}

use crate::godot_component::filter_data::indices::filter_len as ENDPOINT;
use crate::godot_component::filter_data::parse_filter;
const DATA_LEN: usize = (ENDPOINT + 7) / 8;

pub type Filter = FilterFlags<DATA_LEN>;

impl GodotConvert for Filter {
    type Via = Dictionary;
}

impl FromGodot for Filter {
    fn try_from_variant(v: &Variant) -> Result<Self, ConvertError> {
        if v.is_nil() {
            Ok(Self::default())
        } else if v.get_type() == VariantType::String {
            let v = v.try_to()?;
            parse_script(&v).map_err(|e| ConvertError::with_error_value(e, v))
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
        if k.get_type() == VariantType::StringName {
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

        if v.get_type() == VariantType::Bool {
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

            if v.get_type() == VariantType::Bool {
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

impl<'a> Debug for FilterItem<'a> {
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

impl<'a> Display for FilterItem<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        <Self as Debug>::fmt(self, f)
    }
}

impl<'a> Error for FilterItem<'a> {}

#[allow(clippy::type_complexity)]
fn parse_line(
    i: CharSlice<'_>,
) -> IResult<CharSlice<'_>, Option<([Option<&'_ [char]>; 3], bool)>, SingleError<CharSlice<'_>>> {
    let f = |c: char| c.is_alphanumeric() || matches!(c, ':' | '-');

    map(
        delimited(
            space0,
            opt(separated_pair(
                alt((map(tag("deny"), |_| false), map(tag("allow"), |_| true))),
                char_(' '),
                alt((
                    map(char_('*'), |_| [None; 3]),
                    map(
                        pair(
                            take_while1(f),
                            alt((
                                map(tag(".*"), |_: CharSlice| None),
                                opt(preceded(
                                    char_('.'),
                                    pair(
                                        take_while1(f),
                                        alt((
                                            map(tag(".*"), |_: CharSlice| None),
                                            opt(preceded(char_('.'), take_while1(f))),
                                        )),
                                    ),
                                )),
                            )),
                        ),
                        |v| match v {
                            (module, None) => [Some(module.0), None, None],
                            (module, Some((interface, None))) => {
                                [Some(module.0), Some(interface.0), None]
                            }
                            (module, Some((interface, Some(method)))) => {
                                [Some(module.0), Some(interface.0), Some(method.0)]
                            }
                        },
                    ),
                )),
            )),
            char_('\n'),
        ),
        |v| v.map(|(v, f)| (f, v)),
    )(i)
}

fn parse_script(s: &GString) -> Result<Filter, NomErr<SingleError<String>>> {
    let mut s = CharSlice(s.chars());

    let mut ret = Filter::default();
    let mut f = ret.slice_mut(..ENDPOINT);
    let mut module = String::new();
    let mut interface = String::new();
    let mut method = String::new();
    while !s.0.is_empty() {
        let (s_, v) = parse_line(s).map_err(|e| e.map(SingleError::into_owned))?;
        s = s_;

        let Some((t, allow)) = v else {
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

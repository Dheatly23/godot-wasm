use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use anyhow::{bail, Result as AnyResult};
use godot::builtin::meta::{ConvertError, GodotConvert};
use godot::prelude::*;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while1};
use nom::character::complete::{char as char_, space0};
use nom::combinator::{map, opt};
use nom::sequence::{delimited, pair, preceded, separated_pair};
use nom::{Err as NomErr, IResult};

use crate::godot_util::VariantDispatch;
use crate::rw_struct::{CharSlice, SingleError};

#[derive(Default)]
pub struct Filter(SubFilter<SubFilter<SubFilter<bool>>>);
struct SubFilter<T> {
    children: HashMap<Rc<str>, T>,
    default: bool,
}

impl<T> Default for SubFilter<T> {
    fn default() -> Self {
        Self {
            children: HashMap::default(),
            default: true,
        }
    }
}

// SAFETY: No Rc-ed value gets leaked outside.
unsafe impl Send for Filter {}
unsafe impl Sync for Filter {}

impl Filter {
    pub fn filter(&self, module: &str, interface: &str, method: &str) -> bool {
        match self.0.children.get(module) {
            Some(f) => match f.children.get(interface) {
                Some(f) => match f.children.get(method) {
                    Some(&v) => v,
                    None => f.default,
                },
                None => f.default,
            },
            None => self.0.default,
        }
    }

    pub fn pass(&self, module: &str, interface: &str, method: &str) -> AnyResult<()> {
        if self.filter(module, interface, method) {
            Ok(())
        } else {
            bail!("Calling {module}.{interface}.{method} is blocked!")
        }
    }
}

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
    let mut interner: HashMap<Box<[char]>, Rc<str>> = HashMap::new();
    let mut intern = move |s: Variant| {
        let s = s.stringify();
        // SAFETY: Should be safe with Godot 4.1+
        let v = unsafe { s.chars_unchecked() };
        if let Some(r) = interner.get(v) {
            return r.clone();
        }
        let r: Rc<str> = v.iter().collect::<String>().into();
        interner.insert(v.into(), r.clone());
        r
    };

    let mut filter = SubFilter::default();
    for (k, v) in d.iter_shared() {
        let k = intern(k);
        if &*k == "*" {
            filter.default = v.try_to()?;
            continue;
        }

        let v = match VariantDispatch::from(&v) {
            VariantDispatch::Bool(v) => SubFilter {
                default: v,
                ..SubFilter::default()
            },
            VariantDispatch::Dictionary(v) => {
                let mut filter = SubFilter::default();
                for (k, v) in v.iter_shared() {
                    let k = intern(k);
                    if &*k == "*" {
                        filter.default = v.try_to()?;
                        continue;
                    }

                    let v = match VariantDispatch::from(&v) {
                        VariantDispatch::Bool(v) => SubFilter {
                            default: v,
                            ..SubFilter::default()
                        },
                        VariantDispatch::Dictionary(v) => {
                            let mut filter = SubFilter::default();
                            for (k, v) in v.iter_shared() {
                                let v = v.try_to::<bool>()?;
                                let k = intern(k);
                                if &*k == "*" {
                                    filter.default = v;
                                } else {
                                    filter.children.insert(k, v);
                                }
                            }

                            filter
                        }
                        _ => return Err(ConvertError::new(format!("Unknown value {v}"))),
                    };
                    filter.children.insert(k, v);
                }

                filter
            }
            _ => return Err(ConvertError::new(format!("Unknown value {v}"))),
        };
        filter.children.insert(k, v);
    }

    Ok(Filter(filter))
}

enum FilterItem<T> {
    Any,
    Module(T),
    Interface(T, T),
    Method(T, T, T),
}

fn parse_line<'a>(
    i: CharSlice<'a>,
) -> IResult<CharSlice<'a>, Option<(FilterItem<&'a [char]>, bool)>, SingleError<CharSlice<'a>>> {
    let f = |c| matches!(c, 'a'..='z' | 'A'..='Z' | ':' | '-');

    map(
        delimited(
            space0,
            opt(separated_pair(
                alt((map(tag("deny"), |_| false), map(tag("allow"), |_| true))),
                char_(' '),
                alt((
                    map(char_('*'), |_| FilterItem::Any),
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
                            (module, None) => FilterItem::Module(module.0),
                            (module, Some((interface, None))) => {
                                FilterItem::Interface(module.0, interface.0)
                            }
                            (module, Some((interface, Some(method)))) => {
                                FilterItem::Method(module.0, interface.0, method.0)
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
    // SAFETY: Externalize char safety to Godot
    let mut s = unsafe { CharSlice(s.chars_unchecked()) };

    let mut interner = HashSet::new();
    let mut cache = String::new();
    let mut intern = move |s: &[char]| {
        cache.clear();
        cache.extend(s);
        match interner.get(&*cache) {
            None => {
                let r = <Rc<str>>::from(&*cache);
                interner.insert(r.clone());
                r
            }
            Some(v) => v.clone(),
        }
    };

    let mut ret = Filter::default();
    while !s.0.is_empty() {
        let (s_, v) = parse_line(s).map_err(|e| e.map(SingleError::into_owned))?;
        s = s_;

        let Some((f, v)) = v else { continue };
        match f {
            FilterItem::Any => ret.0.default = v,
            FilterItem::Module(module) => {
                ret.0.children.entry(intern(module)).or_default().default = v
            }
            FilterItem::Interface(module, interface) => {
                ret.0
                    .children
                    .entry(intern(module))
                    .or_default()
                    .children
                    .entry(intern(interface))
                    .or_default()
                    .default = v
            }
            FilterItem::Method(module, interface, method) => {
                ret.0
                    .children
                    .entry(intern(module))
                    .or_default()
                    .children
                    .entry(intern(interface))
                    .or_default()
                    .children
                    .insert(intern(method), v);
            }
        }
    }

    Ok(ret)
}

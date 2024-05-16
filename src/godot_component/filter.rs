use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use anyhow::{bail, Result as AnyResult};
use godot::builtin::meta::{ConvertError, GodotConvert};
use godot::prelude::*;

use crate::godot_util::VariantDispatch;

pub enum FilterItem<'a> {
    Any,
    Module(&'a str),
    Interface(&'a str, &'a str),
    Method(&'a str, &'a str, &'a str),
}

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
    fn from_dict(d: Dictionary) -> Result<Self, ConvertError> {
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

        Ok(Self(filter))
    }

    pub fn from_list<'a>(list: impl IntoIterator<Item = (FilterItem<'a>, bool)>) -> Self {
        let mut interner = HashSet::new();
        let mut intern = move |s: &str| match interner.get(s) {
            None => {
                let r = <Rc<str>>::from(s);
                interner.insert(r.clone());
                r
            }
            Some(v) => v.clone(),
        };

        let mut ret = Self(SubFilter::default());
        let filter = &mut ret.0;
        for (f, v) in list {
            match f {
                FilterItem::Any => filter.default = v,
                FilterItem::Module(module) => {
                    filter.children.entry(intern(module)).or_default().default = v
                }
                FilterItem::Interface(module, interface) => {
                    filter
                        .children
                        .entry(intern(module))
                        .or_default()
                        .children
                        .entry(intern(interface))
                        .or_default()
                        .default = v
                }
                FilterItem::Method(module, interface, method) => {
                    filter
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

        ret
    }

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
            return Ok(Self::default());
        }
        Self::from_dict(v.try_to()?)
    }

    fn try_from_godot(via: Self::Via) -> Result<Self, ConvertError> {
        Self::from_dict(via)
    }

    fn from_godot(via: Self::Via) -> Self {
        Self::from_dict(via).unwrap()
    }
}

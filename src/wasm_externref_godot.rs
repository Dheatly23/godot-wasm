use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Linker, Trap};

/// Godot module name
pub const GODOT_MODULE: &str = "godot";

#[inline]
pub fn variant_to_externref(object: Variant) -> Option<ExternRef> {
    if object.is_nil() {
        None
    } else {
        Some(ExternRef::new(object))
    }
}

#[inline]
pub fn externref_to_variant(ext: Option<ExternRef>) -> Result<Variant, Trap> {
    ext.map_or_else(
        || Ok(Variant::new()),
        |v| {
            v.data()
                .downcast_ref::<Variant>()
                .cloned()
                .ok_or_else(|| Trap::new("External reference is not a Godot variant"))
        },
    )
}

#[inline(always)]
fn externref_to_variant_nonnull(ext: Option<ExternRef>) -> Result<Variant, Trap> {
    ext.ok_or_else(|| Trap::new("Null value")).and_then(|v| {
        v.data()
            .downcast_ref::<Variant>()
            .cloned()
            .ok_or_else(|| Trap::new("External reference is not a Godot variant"))
    })
}

#[inline(always)]
fn externref_to_object<T: FromVariant>(ext: Option<ExternRef>) -> Result<T, Trap> {
    externref_to_variant_nonnull(ext)
        .and_then(|v| T::from_variant(&v).map_err(|e| Trap::from(Box::new(e) as Box<_>)))
}

macro_rules! variant_convert {
    ($l:ident, $t:ty, ($from:literal, $to:literal)) => {{
        $l.func_wrap(GODOT_MODULE, $from, |v: $t| {
            variant_to_externref(v.to_variant())
        })?;

        $l.func_wrap(GODOT_MODULE, $to, externref_to_object::<$t>)?;
    }};
}

macro_rules! variant_typecheck {
    ($l:ident, $t:ty, $is:literal) => {
        $l.func_wrap(GODOT_MODULE, $is, |v: Option<ExternRef>| {
            v.and_then(|v| {
                v.data()
                    .downcast_ref::<Variant>()
                    .and_then(|v| <$t>::from_variant(&v).ok())
            })
            .is_some() as i32
        })?
    };
}

macro_rules! object_new {
    ($l:ident, $t:ty, $new:literal) => {
        $l.func_wrap(GODOT_MODULE, $new, || {
            variant_to_externref(<$t>::new().owned_to_variant())
        })?
    };
}

macro_rules! object_dup {
    ($l:ident, $t:ty, $dup:literal) => {
        $l.func_wrap(GODOT_MODULE, $dup, |v| {
            externref_to_object::<$t>(v)
                .map(|v| variant_to_externref(v.duplicate().owned_to_variant()))
        })?
    };
}

/// Register godot module
pub fn register_godot_externref<T>(linker: &mut Linker<T>) -> anyhow::Result<()> {
    linker.func_wrap(GODOT_MODULE, "var.is_var", |v: Option<ExternRef>| {
        v.map(|v| v.data().downcast_ref::<Variant>().is_some())
            .unwrap_or(false) as i32
    })?;

    variant_typecheck!(linker, i32, "var.is_i32");
    variant_typecheck!(linker, i64, "var.is_i64");
    variant_typecheck!(linker, f32, "var.is_f32");
    variant_typecheck!(linker, f64, "var.is_f64");
    variant_typecheck!(linker, VariantArray, "var.is_array");
    variant_typecheck!(linker, Dictionary, "var.is_dictionary");
    variant_typecheck!(linker, GodotString, "var.is_string");
    variant_typecheck!(linker, Ref<Object>, "var.is_object");

    variant_convert!(linker, i32, ("var.from_i32", "var.to_i32"));
    variant_convert!(linker, i64, ("var.from_i64", "var.to_i64"));
    variant_convert!(linker, f32, ("var.from_f32", "var.to_f32"));
    variant_convert!(linker, f64, ("var.from_f64", "var.to_f64"));

    object_new!(linker, VariantArray<Unique>, "arr.create");
    object_new!(linker, Dictionary<Unique>, "dict.create");

    object_dup!(linker, VariantArray, "arr.duplicate");
    object_dup!(linker, Dictionary, "dict.duplicate");

    linker.func_wrap(GODOT_MODULE, "arr.size", |v| {
        externref_to_object::<VariantArray>(v).map(|v| v.len())
    })?;

    linker.func_wrap(GODOT_MODULE, "arr.get", |i, v| {
        externref_to_object::<VariantArray>(v).and_then(|v| {
            if (i < 0) || (i >= v.len()) {
                Err(Trap::new("Out of bound"))
            } else {
                Ok(variant_to_externref(v.get(i)))
            }
        })
    })?;

    linker.func_wrap(GODOT_MODULE, "arr.set", |i, x, v| {
        externref_to_object::<VariantArray>(v)
            .and_then(|v| externref_to_variant(x).map(|x| (v, x)))
            .and_then(|(v, x)| {
                if (i < 0) || (i >= v.len()) {
                    Err(Trap::new("Out of bound"))
                } else {
                    Ok(v.set(i, x))
                }
            })
    })?;

    linker.func_wrap(GODOT_MODULE, "arr.grow", |x, n: i32, v| {
        externref_to_object::<VariantArray>(v)
            .and_then(|v| externref_to_variant(x).map(|x| (v, x)))
            .map(|(v, x)| {
                let v = unsafe { v.assume_unique() };
                if n > 0 {
                    for _ in 0..n {
                        v.push(x.clone());
                    }
                } else if n < 0 {
                    v.resize(v.len() - n);
                }
                v.len()
            })
    })?;

    linker.func_wrap(GODOT_MODULE, "arr.fill", |i: i32, x, n: i32, v| {
        externref_to_object::<VariantArray>(v)
            .and_then(|v| {
                if (n < 0) || (i < 0) || ((i + n) > v.len()) {
                    return Err(Trap::new("Out of bound"));
                }
                externref_to_variant(x).map(|x| (v, x))
            })
            .map(|(v, x)| {
                for j in i..(i + n) {
                    v.set(j, x.clone());
                }
            })
    })?;

    linker.func_wrap(GODOT_MODULE, "dict.size", |d| {
        externref_to_object::<Dictionary>(d).map(|v| v.len())
    })?;

    linker.func_wrap(GODOT_MODULE, "dict.get", |k, d| {
        externref_to_object::<Dictionary>(d)
            .and_then(|d| externref_to_variant(k).map(|k| (d, k)))
            .map(|(d, k)| variant_to_externref(d.get(k)))
    })?;

    linker.func_wrap(GODOT_MODULE, "dict.set", |k, v, d| {
        externref_to_object::<Dictionary>(d)
            .and_then(|d| externref_to_variant(k).map(|k| (d, k)))
            .and_then(|(d, k)| externref_to_variant(v).map(|v| (d, k, v)))
            .map(|(d, k, v)| d.update(k, v))
    })?;

    linker.func_wrap(GODOT_MODULE, "dict.delete", |k, d| {
        externref_to_object::<Dictionary>(d)
            .and_then(|d| externref_to_variant(k).map(|k| (d, k)))
            .map(|(d, k)| unsafe { d.assume_unique() }.erase(k))
    })?;

    linker.func_wrap(GODOT_MODULE, "dict.key_in", |k, d| {
        externref_to_object::<Dictionary>(d)
            .and_then(|d| externref_to_variant(k).map(|k| (d, k)))
            .map(|(d, k)| d.contains(k) as i32)
    })?;

    linker.func_wrap(
        GODOT_MODULE,
        "dict.iter",
        |mut ctx: Caller<_>, f: Option<Func>, d| {
            externref_to_object::<Dictionary>(d)
                .and_then(|d| {
                    f.ok_or_else(|| Trap::new("Function is null"))
                        .and_then(|f| {
                            f.typed::<(Option<ExternRef>, Option<ExternRef>), i32, _>(&ctx)
                                .map_err(Trap::from)
                        })
                        .map(|f| (d, f))
                })
                .and_then(|(d, f)| {
                    for (k, v) in d.iter() {
                        if f.call(&mut ctx, (variant_to_externref(k), variant_to_externref(v)))?
                            != 0
                        {
                            break;
                        }
                    }
                    Ok(())
                })
        },
    )?;

    Ok(())
}

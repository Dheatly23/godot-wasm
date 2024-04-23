use anyhow::Result as AnyResult;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Rooted, StoreContextMut, TypedFunc};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "object.",
    has_method => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(&ctx, name)?))?;
        let obj = externref_to_variant(&ctx, obj)?;
        Ok(obj.has_method(name).into())
    },
    call => |mut ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(&ctx, name)?))?;
        let mut obj = externref_to_variant(&ctx, obj)?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(&ctx, e)?);
                if n == 0 {
                    break;
                }
            }
        }

        // SAFETY: We want to call to Godot
        variant_to_externref(ctx, unsafe {
            site_context!(obj.call(name, &v))
        }?)
    },
    call_deferred => |mut ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, f: Option<Func>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(&ctx, name)?))?;
        let obj = externref_to_variant(&ctx, obj)?;

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<Rooted<ExternRef>>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(&ctx, e)?);
                if n == 0 {
                    break;
                }
            }
        }

        let r = if let Ok(o) = <Ref<Reference>>::from_variant(&obj) {
            // SAFETY: This is a reference object, so should be safe.
            unsafe { o.assume_safe().call_deferred(name, &v) }
        } else {
            let o = site_context!(<Ref<Object>>::from_variant(&obj))?;
            // SAFETY: Use assume_safe_if_sane(), which at least prevent some of unsafety.
            unsafe {
                match o.assume_safe_if_sane() {
                    Some(o) => o.call_deferred(name, &v),
                    None => Variant::nil(),
                }
            }
        };

        variant_to_externref(ctx, r)
    },
    get => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>| -> AnyResult<Option<Rooted<ExternRef>>> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(&ctx, name)?))?;
        let obj = externref_to_variant(&ctx, obj)?;

        let r = if let Ok(o) = <Ref<Reference>>::from_variant(&obj) {
            // SAFETY: This is a reference object, so should be safe.
            unsafe { o.assume_safe().get(name) }
        } else {
            let o = site_context!(<Ref<Object>>::from_variant(&obj))?;
            // SAFETY: Use assume_safe_if_sane(), which at least prevent some of unsafety.
            unsafe {
                match o.assume_safe_if_sane() {
                    Some(o) => o.get(name),
                    None => Variant::nil(),
                }
            }
        };

        variant_to_externref(ctx, r)
    },
    set => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, name: Option<Rooted<ExternRef>>, value: Option<Rooted<ExternRef>>| -> AnyResult<u32> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(&ctx, name)?))?;
        let obj = externref_to_variant(&ctx, obj)?;
        let value = externref_to_variant(&ctx, value)?;

        if let Ok(o) = <Ref<Reference>>::from_variant(&obj) {
            // SAFETY: This is a reference object, so should be safe.
            unsafe { o.assume_safe().set(name, value) }
        } else {
            let o = site_context!(<Ref<Object>>::from_variant(&obj))?;
            // SAFETY: Use assume_safe_if_sane(), which at least prevent some of unsafety.
            unsafe {
                match o.assume_safe_if_sane() {
                    Some(o) => o.set(name, value),
                    None => return Ok(0),
                }
            }
        }

        Ok(1)
    },
    connect => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, signal: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>, method: Option<Rooted<ExternRef>>, binds: Option<Rooted<ExternRef>>, flags: i64| -> AnyResult<()> {
        let signal =
            site_context!(GodotString::from_variant(&externref_to_variant(&ctx, signal)?))?;
        let method =
            site_context!(GodotString::from_variant(&externref_to_variant(&ctx, method)?))?;
        let binds =
            site_context!(VariantArray::from_variant(&externref_to_variant(&ctx, binds)?))?;
        let obj = externref_to_variant(&ctx, obj)?;
        let target = externref_to_variant(&ctx, target)?;

        if let Ok(o) = <Ref<Reference>>::from_variant(&obj) {
            if let Ok(target) = <Ref<Reference>>::from_variant(&target) {
                // SAFETY: This is a reference object, so should be safe.
                unsafe {
                    o.assume_safe()
                        .connect(signal, target, method, binds, flags)?
                }
            } else {
                let target = site_context!(<Ref<Object>>::from_variant(&target))?;
                // SAFETY: This is a reference object, so should be safe.
                unsafe {
                    o.assume_safe()
                        .connect(signal, target, method, binds, flags)?
                }
            }
        } else {
            let o = site_context!(<Ref<Object>>::from_variant(&obj))?;
            // SAFETY: Use assume_safe_if_sane(), which at least prevent some of unsafety.
            unsafe {
                match o.assume_safe_if_sane() {
                    Some(o) => {
                        if let Ok(target) = <Ref<Reference>>::from_variant(&target) {
                            o.connect(signal, target, method, binds, flags)?
                        } else {
                            let target =
                                site_context!(<Ref<Object>>::from_variant(&target))?;
                            o.connect(signal, target, method, binds, flags)?
                        }
                    }
                    None => bail_with_site!("Object is invalid!"),
                }
            }
        }

        Ok(())
    },
    disconnect => |ctx: Caller<'_, _>, obj: Option<Rooted<ExternRef>>, signal: Option<Rooted<ExternRef>>, target: Option<Rooted<ExternRef>>, method: Option<Rooted<ExternRef>>| -> AnyResult<()> {
        let signal =
            site_context!(GodotString::from_variant(&externref_to_variant(&ctx, signal)?))?;
        let method =
            site_context!(GodotString::from_variant(&externref_to_variant(&ctx, method)?))?;
        let obj = externref_to_variant(&ctx, obj)?;
        let target = externref_to_variant(&ctx, target)?;

        if let Ok(o) = <Ref<Reference>>::from_variant(&obj) {
            if let Ok(target) = <Ref<Reference>>::from_variant(&target) {
                // SAFETY: This is a reference object, so should be safe.
                unsafe { o.assume_safe().disconnect(signal, target, method) }
            } else {
                let target = site_context!(<Ref<Object>>::from_variant(&target))?;
                // SAFETY: This is a reference object, so should be safe.
                unsafe { o.assume_safe().disconnect(signal, target, method) }
            }
        } else {
            let o = site_context!(<Ref<Object>>::from_variant(&obj))?;
            // SAFETY: Use assume_safe_if_sane(), which at least prevent some of unsafety.
            unsafe {
                match o.assume_safe_if_sane() {
                    Some(o) => {
                        if let Ok(target) = <Ref<Reference>>::from_variant(&target) {
                            o.disconnect(signal, target, method)
                        } else {
                            let target =
                                site_context!(<Ref<Object>>::from_variant(&target))?;
                            o.disconnect(signal, target, method)
                        }
                    }
                    None => bail_with_site!("Object is invalid!"),
                }
            }
        }

        Ok(())
    },
}

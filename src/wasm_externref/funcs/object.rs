use anyhow::Error;
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Func, StoreContextMut, TypedFunc};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::{bail_with_site, func_registry, site_context};

func_registry! {
    "object.",
    has_method => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<u32, Error> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(name)))?;
        let obj = externref_to_variant(obj);
        Ok(obj.has_method(name).into())
    },
    call => |mut ctx: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, f: Option<Func>| -> Result<Option<ExternRef>, Error> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(name)))?;
        let mut obj = externref_to_variant(obj);

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<ExternRef>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(e));
                if n == 0 {
                    break;
                }
            }
        }

        // SAFETY: We want to call to Godot
        Ok(variant_to_externref(unsafe {
            site_context!(obj.call(name, &v))
        }?))
    },
    call_deferred => |mut ctx: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, f: Option<Func>| -> Result<Option<ExternRef>, Error> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(name)))?;
        let obj = externref_to_variant(obj);

        let mut v = Vec::new();
        if let Some(f) = f {
            let f: TypedFunc<u32, (Option<ExternRef>, u32)> = site_context!(f.typed(&ctx))?;
            loop {
                let (e, n) = site_context!(f.call(&mut ctx, v.len() as _))?;
                v.push(externref_to_variant(e));
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

        Ok(variant_to_externref(r))
    },
    get => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<Option<ExternRef>, Error> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(name)))?;
        let obj = externref_to_variant(obj);

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

        Ok(variant_to_externref(r))
    },
    set => |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>, value: Option<ExternRef>| -> Result<u32, Error> {
        let name = site_context!(GodotString::from_variant(&externref_to_variant(name)))?;
        let obj = externref_to_variant(obj);
        let value = externref_to_variant(value);

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
    connect => |_: Caller<_>, obj: Option<ExternRef>, signal: Option<ExternRef>, target: Option<ExternRef>, method: Option<ExternRef>, binds: Option<ExternRef>, flags: i64| -> Result<(), Error> {
        let signal =
            site_context!(GodotString::from_variant(&externref_to_variant(signal)))?;
        let method =
            site_context!(GodotString::from_variant(&externref_to_variant(method)))?;
        let binds =
            site_context!(VariantArray::from_variant(&externref_to_variant(binds)))?;
        let obj = externref_to_variant(obj);
        let target = externref_to_variant(target);

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
    disconnect => |_: Caller<_>, obj: Option<ExternRef>, signal: Option<ExternRef>, target: Option<ExternRef>, method: Option<ExternRef>| -> Result<(), Error> {
        let signal =
            site_context!(GodotString::from_variant(&externref_to_variant(signal)))?;
        let method =
            site_context!(GodotString::from_variant(&externref_to_variant(method)))?;
        let obj = externref_to_variant(obj);
        let target = externref_to_variant(target);

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

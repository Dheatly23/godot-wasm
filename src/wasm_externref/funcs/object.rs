use anyhow::{bail, Error};
use gdnative::prelude::*;
use wasmtime::{Caller, ExternRef, Func, Linker, TypedFunc};

use crate::wasm_externref::{externref_to_variant, variant_to_externref};
use crate::wasm_instance::StoreData;
use crate::wasm_util::EXTERNREF_MODULE;

#[inline]
pub fn register_functions(linker: &mut Linker<StoreData>) {
    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "object.has_method",
            |_: Caller<_>, obj: Option<ExternRef>, name: Option<ExternRef>| -> Result<u32, Error> {
                let name = GodotString::from_variant(&externref_to_variant(name))?;
                let obj = externref_to_variant(obj);
                Ok(obj.has_method(name).into())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "object.call",
            |mut ctx: Caller<_>,
             obj: Option<ExternRef>,
             name: Option<ExternRef>,
             f: Option<Func>|
             -> Result<Option<ExternRef>, Error> {
                let name = GodotString::from_variant(&externref_to_variant(name))?;
                let mut obj = externref_to_variant(obj);

                let mut v = Vec::new();
                if let Some(f) = f {
                    let f: TypedFunc<u32, (Option<ExternRef>, u32)> = f.typed(&ctx)?;
                    loop {
                        let (e, n) = f.call(&mut ctx, v.len() as _)?;
                        v.push(externref_to_variant(e));
                        if n == 0 {
                            break;
                        }
                    }
                }

                // SAFETY: We want to call to Godot
                Ok(variant_to_externref(unsafe { obj.call(name, &v) }?))
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "object.call_deferred",
            |mut ctx: Caller<_>,
             obj: Option<ExternRef>,
             name: Option<ExternRef>,
             f: Option<Func>|
             -> Result<Option<ExternRef>, Error> {
                let name = GodotString::from_variant(&externref_to_variant(name))?;
                let obj = externref_to_variant(obj);

                let mut v = Vec::new();
                if let Some(f) = f {
                    let f: TypedFunc<u32, (Option<ExternRef>, u32)> = f.typed(&ctx)?;
                    loop {
                        let (e, n) = f.call(&mut ctx, v.len() as _)?;
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
                    let o = <Ref<Object>>::from_variant(&obj)?;
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
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "object.get",
            |_: Caller<_>,
             obj: Option<ExternRef>,
             name: Option<ExternRef>|
             -> Result<Option<ExternRef>, Error> {
                let name = GodotString::from_variant(&externref_to_variant(name))?;
                let obj = externref_to_variant(obj);

                let r = if let Ok(o) = <Ref<Reference>>::from_variant(&obj) {
                    // SAFETY: This is a reference object, so should be safe.
                    unsafe { o.assume_safe().get(name) }
                } else {
                    let o = <Ref<Object>>::from_variant(&obj)?;
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
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "object.set",
            |_: Caller<_>,
             obj: Option<ExternRef>,
             name: Option<ExternRef>,
             value: Option<ExternRef>|
             -> Result<u32, Error> {
                let name = GodotString::from_variant(&externref_to_variant(name))?;
                let obj = externref_to_variant(obj);
                let value = externref_to_variant(value);

                if let Ok(o) = <Ref<Reference>>::from_variant(&obj) {
                    // SAFETY: This is a reference object, so should be safe.
                    unsafe { o.assume_safe().set(name, value) }
                } else {
                    let o = <Ref<Object>>::from_variant(&obj)?;
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
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "object.connect",
            |_: Caller<_>,
             obj: Option<ExternRef>,
             signal: Option<ExternRef>,
             target: Option<ExternRef>,
             method: Option<ExternRef>,
             binds: Option<ExternRef>,
             flags: i64|
             -> Result<(), Error> {
                let signal = GodotString::from_variant(&externref_to_variant(signal))?;
                let method = GodotString::from_variant(&externref_to_variant(method))?;
                let binds = VariantArray::from_variant(&externref_to_variant(binds))?;
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
                        let target = <Ref<Object>>::from_variant(&target)?;
                        // SAFETY: This is a reference object, so should be safe.
                        unsafe {
                            o.assume_safe()
                                .connect(signal, target, method, binds, flags)?
                        }
                    }
                } else {
                    let o = <Ref<Object>>::from_variant(&obj)?;
                    // SAFETY: Use assume_safe_if_sane(), which at least prevent some of unsafety.
                    unsafe {
                        match o.assume_safe_if_sane() {
                            Some(o) => {
                                if let Ok(target) = <Ref<Reference>>::from_variant(&target) {
                                    o.connect(signal, target, method, binds, flags)?
                                } else {
                                    let target = <Ref<Object>>::from_variant(&target)?;
                                    o.connect(signal, target, method, binds, flags)?
                                }
                            }
                            None => bail!("Object is invalid!"),
                        }
                    }
                }

                Ok(())
            },
        )
        .unwrap();

    linker
        .func_wrap(
            EXTERNREF_MODULE,
            "object.disconnect",
            |_: Caller<_>,
             obj: Option<ExternRef>,
             signal: Option<ExternRef>,
             target: Option<ExternRef>,
             method: Option<ExternRef>|
             -> Result<(), Error> {
                let signal = GodotString::from_variant(&externref_to_variant(signal))?;
                let method = GodotString::from_variant(&externref_to_variant(method))?;
                let obj = externref_to_variant(obj);
                let target = externref_to_variant(target);

                if let Ok(o) = <Ref<Reference>>::from_variant(&obj) {
                    if let Ok(target) = <Ref<Reference>>::from_variant(&target) {
                        // SAFETY: This is a reference object, so should be safe.
                        unsafe { o.assume_safe().disconnect(signal, target, method) }
                    } else {
                        let target = <Ref<Object>>::from_variant(&target)?;
                        // SAFETY: This is a reference object, so should be safe.
                        unsafe { o.assume_safe().disconnect(signal, target, method) }
                    }
                } else {
                    let o = <Ref<Object>>::from_variant(&obj)?;
                    // SAFETY: Use assume_safe_if_sane(), which at least prevent some of unsafety.
                    unsafe {
                        match o.assume_safe_if_sane() {
                            Some(o) => {
                                if let Ok(target) = <Ref<Reference>>::from_variant(&target) {
                                    o.disconnect(signal, target, method)
                                } else {
                                    let target = <Ref<Object>>::from_variant(&target)?;
                                    o.disconnect(signal, target, method)
                                }
                            }
                            None => bail!("Object is invalid!"),
                        }
                    }
                }

                Ok(())
            },
        )
        .unwrap();
}

use anyhow::Result as AnyResult;
use godot::classes::Marshalls;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;

filter_macro! {method [
    singleton -> "singleton",
    base64_to_raw -> "base64-to-raw",
    raw_to_base64 -> "raw-to-base64",
    base64_to_utf8 -> "base64-to-utf8",
    utf8_to_base64 -> "utf8-to-base64",
    base64_to_variant -> "base64-to-variant",
    variant_to_base64 -> "variant-to-base64",
    base64_to_variant_with_objects -> "base64-to-variant-with-objects",
    variant_to_base64_with_objects -> "variant-to-base64-with-objects",
]}

impl crate::godot_component::bindgen::godot::global::marshalls::Host
    for crate::godot_component::GodotCtx
{
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, marshalls, singleton)?;
        self.set_into_var(Marshalls::singleton())
    }

    fn base64_to_raw(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, marshalls, base64_to_raw)?;
        let r = Marshalls::singleton().base64_to_raw(&self.get_value::<GString>(var)?);
        self.set_into_var(r)
    }

    fn raw_to_base64(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, marshalls, raw_to_base64)?;
        let r = Marshalls::singleton().raw_to_base64(&self.get_value(var)?);
        self.set_into_var(r)
    }

    fn base64_to_utf8(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, marshalls, base64_to_utf8)?;
        let r = Marshalls::singleton().base64_to_utf8(&self.get_value::<GString>(var)?);
        self.set_into_var(r)
    }

    fn utf8_to_base64(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, marshalls, utf8_to_base64)?;
        let r = Marshalls::singleton().utf8_to_base64(&self.get_value::<GString>(var)?);
        self.set_into_var(r)
    }

    fn base64_to_variant(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, marshalls, base64_to_variant)?;
        let v: GString = self.get_value(var)?;
        let r = self.release_store(move || {
            Marshalls::singleton()
                .base64_to_variant_ex(&v)
                .allow_objects(false)
                .done()
        });
        self.set_var(r)
    }

    fn variant_to_base64(
        &mut self,
        var: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, marshalls, variant_to_base64)?;
        let v = self.maybe_get_var(var)?;
        let r = self.release_store(move || {
            Marshalls::singleton()
                .variant_to_base64_ex(&v)
                .full_objects(false)
                .done()
        });
        self.set_into_var(r)
    }

    fn base64_to_variant_with_objects(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, marshalls, base64_to_variant_with_objects)?;
        let v: GString = self.get_value(var)?;
        let r = self.release_store(move || {
            Marshalls::singleton()
                .base64_to_variant_ex(&v)
                .allow_objects(true)
                .done()
        });
        self.set_var(r)
    }

    fn variant_to_base64_with_objects(
        &mut self,
        var: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, marshalls, variant_to_base64_with_objects)?;
        let v = self.maybe_get_var(var)?;
        let r = self.release_store(move || {
            Marshalls::singleton()
                .variant_to_base64_ex(&v)
                .full_objects(true)
                .done()
        });
        self.set_into_var(r)
    }
}

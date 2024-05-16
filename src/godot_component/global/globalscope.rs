use anyhow::{bail, Result as AnyResult};
use godot::engine::utilities::*;
use godot::engine::{ResourceLoader, ResourceSaver};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};
use crate::godot_util::from_var_any;
use crate::site_context;

impl<T: AsMut<GodotCtx>> bindgen::godot::global::globalscope::Host for T {
    fn print(&mut self, s: String) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "print"))?;
        godot::engine::utilities::print(s.to_variant(), &[]);
        Ok(())
    }

    fn print_rich(&mut self, s: String) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "print-rich"))?;
        godot::engine::utilities::print_rich(s.to_variant(), &[]);
        Ok(())
    }

    fn printerr(&mut self, s: String) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "printerr"))?;
        printerr(s.to_variant(), &[]);
        Ok(())
    }

    fn push_error(&mut self, s: String) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "push-error"))?;
        push_error(s.to_variant(), &[]);
        Ok(())
    }

    fn push_warning(&mut self, s: String) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "push-warning"))?;
        push_warning(s.to_variant(), &[]);
        Ok(())
    }

    fn bytes_to_var(
        &mut self,
        b: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "bytes-to-var"))?;
        let v = bytes_to_var(from_var_any(this.get_var_borrow(b)?)?);
        this.set_var(v)
    }

    fn bytes_to_var_with_objects(
        &mut self,
        b: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass(
            "godot:global",
            "globalscope",
            "bytes-to-var-with-objects"
        ))?;
        let v = bytes_to_var_with_objects(from_var_any(this.get_var_borrow(b)?)?);
        this.set_var(v)
    }

    fn var_to_bytes(
        &mut self,
        v: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "var-to-bytes"))?;
        let b = var_to_bytes(this.maybe_get_var(v)?);
        this.set_into_var(b)
    }

    fn var_to_bytes_with_objects(
        &mut self,
        v: Option<WasmResource<Variant>>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass(
            "godot:global",
            "globalscope",
            "var-to-bytes-with-objects"
        ))?;
        let b = var_to_bytes_with_objects(this.maybe_get_var(v)?);
        this.set_into_var(b)
    }

    fn var_to_str(&mut self, v: Option<WasmResource<Variant>>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "var-to-str"))?;
        let s = var_to_str(this.maybe_get_var(v)?);
        this.set_into_var(s)
    }

    fn str_to_var(&mut self, s: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "str-to-var"))?;
        let v = str_to_var(from_var_any(this.get_var_borrow(s)?)?);
        this.set_var(v)
    }

    fn weakref(&mut self, v: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "weakref"))?;
        let v = weakref(this.get_var(v)?);
        this.set_var(v)
    }

    fn is_instance_valid(&mut self, v: WasmResource<Variant>) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "is-instance-valid"))?;
        Ok(is_instance_valid(this.get_var(v)?))
    }

    fn is_instance_id_valid(&mut self, id: u64) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "is-instance-id-valid"))?;
        Ok(is_instance_id_valid(id as _))
    }

    fn is_same(&mut self, a: WasmResource<Variant>, b: WasmResource<Variant>) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "is-same"))?;
        Ok(is_same(this.get_var(a)?, this.get_var(b)?))
    }

    fn rand_from_seed(&mut self, seed: u64) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "rand-from-seed"))?;
        this.set_into_var(rand_from_seed(seed as _))
    }

    fn randf(&mut self) -> AnyResult<f64> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "randf"))?;
        Ok(randf())
    }

    fn randf_range(&mut self, from: f64, to: f64) -> AnyResult<f64> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "randf-range"))?;
        Ok(randf_range(from, to))
    }

    fn randfn(&mut self, mean: f64, deviation: f64) -> AnyResult<f64> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "randfn"))?;
        Ok(randfn(mean, deviation))
    }

    fn randi(&mut self) -> AnyResult<i64> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "randi"))?;
        Ok(randi())
    }

    fn randi_range(&mut self, from: i64, to: i64) -> AnyResult<i64> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "globalscope", "randi-range"))?;
        Ok(randi_range(from, to))
    }

    fn randomize(&mut self) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "randomize"))?;
        randomize();
        Ok(())
    }

    fn seed(&mut self, s: u64) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "seed"))?;
        seed(s as _);
        Ok(())
    }

    fn load(&mut self, path: String) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "load"))?;
        match ResourceLoader::singleton().load((&path).into()) {
            Some(v) => this.set_into_var(v),
            None => bail!("Cannot load resource {path}"),
        }
    }

    fn save(&mut self, res: WasmResource<Variant>, path: String) -> ErrorRes {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "globalscope", "save"))?;
        wrap_error(
            ResourceSaver::singleton()
                .save_ex(from_var_any(this.get_var_borrow(res)?)?)
                .path((&path).into())
                .done(),
        )
    }
}

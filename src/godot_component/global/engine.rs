use anyhow::Result as AnyResult;
use godot::engine::Engine;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use super::gate_unsafe;
use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};
use crate::godot_util::from_var_any;

impl<T: AsMut<GodotCtx>> bindgen::godot::global::engine::Host for T {
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        gate_unsafe(&*this)?;
        this.set_into_var(Engine::singleton())
    }

    fn get_max_fps(&mut self) -> AnyResult<i32> {
        Ok(Engine::singleton().get_max_fps())
    }

    fn get_max_physics_steps_per_frame(&mut self) -> AnyResult<i32> {
        Ok(Engine::singleton().get_max_physics_steps_per_frame())
    }

    fn get_physics_jitter_fix(&mut self) -> AnyResult<f64> {
        Ok(Engine::singleton().get_physics_jitter_fix())
    }

    fn get_physics_ticks_per_second(&mut self) -> AnyResult<i32> {
        Ok(Engine::singleton().get_physics_ticks_per_second())
    }

    fn is_printing_error_messages(&mut self) -> AnyResult<bool> {
        Ok(Engine::singleton().is_printing_error_messages())
    }

    fn get_time_scale(&mut self) -> AnyResult<f64> {
        Ok(Engine::singleton().get_time_scale())
    }

    fn get_architecture_name(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(Engine::singleton().get_architecture_name())
    }

    fn get_author_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(Engine::singleton().get_author_info())
    }

    fn get_copyright_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(Engine::singleton().get_copyright_info())
    }

    fn get_donor_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(Engine::singleton().get_donor_info())
    }

    fn get_frames_drawn(&mut self) -> AnyResult<i32> {
        Ok(Engine::singleton().get_frames_drawn())
    }

    fn get_frames_per_second(&mut self) -> AnyResult<f64> {
        Ok(Engine::singleton().get_frames_per_second())
    }

    fn get_license_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(Engine::singleton().get_license_info())
    }

    fn get_license_text(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(Engine::singleton().get_license_text())
    }

    fn get_main_loop(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        gate_unsafe(&*this)?;

        this.set_into_var(Engine::singleton().get_main_loop())
    }

    fn get_physics_frames(&mut self) -> AnyResult<u64> {
        Ok(Engine::singleton().get_physics_frames())
    }

    fn get_physics_interpolation_fraction(&mut self) -> AnyResult<f64> {
        Ok(Engine::singleton().get_physics_interpolation_fraction())
    }

    fn get_process_frames(&mut self) -> AnyResult<u64> {
        Ok(Engine::singleton().get_process_frames())
    }

    fn get_script_language(&mut self, ix: i32) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        gate_unsafe(&*this)?;

        Engine::singleton()
            .get_script_language(ix)
            .map(|v| this.set_into_var(v))
            .transpose()
    }

    fn get_script_language_count(&mut self) -> AnyResult<i32> {
        Ok(Engine::singleton().get_script_language_count())
    }

    fn get_singleton(
        &mut self,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        gate_unsafe(&*this)?;

        let name: StringName = from_var_any(this.get_var_borrow(name)?)?;
        Engine::singleton()
            .get_singleton(name)
            .map(|v| this.set_into_var(v))
            .transpose()
    }

    fn get_singleton_list(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(Engine::singleton().get_singleton_list())
    }

    fn get_version_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(Engine::singleton().get_version_info())
    }

    fn get_write_movie_path(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut()
            .set_into_var(Engine::singleton().get_write_movie_path())
    }

    fn has_singleton(&mut self, name: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(Engine::singleton().has_singleton(from_var_any(self.as_mut().get_var_borrow(name)?)?))
    }

    fn is_editor_hint(&mut self) -> AnyResult<bool> {
        Ok(Engine::singleton().is_editor_hint())
    }

    fn is_in_physics_frame(&mut self) -> AnyResult<bool> {
        Ok(Engine::singleton().is_in_physics_frame())
    }

    fn register_script_language(&mut self, lang: WasmResource<Variant>) -> ErrorRes {
        let this = self.as_mut();
        gate_unsafe(&*this)?;

        wrap_error(
            Engine::singleton().register_script_language(from_var_any(this.get_var_borrow(lang)?)?),
        )
    }

    fn unregister_script_language(&mut self, lang: WasmResource<Variant>) -> ErrorRes {
        let this = self.as_mut();
        gate_unsafe(&*this)?;

        wrap_error(
            Engine::singleton().register_script_language(from_var_any(this.get_var_borrow(lang)?)?),
        )
    }

    fn register_singleton(
        &mut self,
        name: WasmResource<Variant>,
        inst: WasmResource<Variant>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        gate_unsafe(&*this)?;

        let name: StringName = from_var_any(this.get_var_borrow(name)?)?;
        let inst: Gd<Object> = from_var_any(this.get_var_borrow(inst)?)?;
        Engine::singleton().register_singleton(name, inst);
        Ok(())
    }

    fn unregister_singleton(&mut self, name: WasmResource<Variant>) -> AnyResult<()> {
        let this = self.as_mut();
        gate_unsafe(&*this)?;

        Engine::singleton().unregister_singleton(from_var_any(this.get_var_borrow(name)?)?);
        Ok(())
    }
}

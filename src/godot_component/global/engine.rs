use anyhow::Result as AnyResult;
use godot::engine::Engine;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::{bindgen, wrap_error, ErrorRes, GodotCtx};
use crate::site_context;

impl<T: AsMut<GodotCtx>> bindgen::godot::global::engine::Host for T {
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "singleton"))?;
        this.set_into_var(Engine::singleton())
    }

    fn get_max_fps(&mut self) -> AnyResult<i32> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "get-max-fps"))?;
        Ok(Engine::singleton().get_max_fps())
    }

    fn set_max_fps(&mut self, v: i32) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "set-max-fps"))?;
        Engine::singleton().set_max_fps(v);
        Ok(())
    }

    fn get_max_physics_steps_per_frame(&mut self) -> AnyResult<i32> {
        let this = self.as_mut();
        site_context!(this.filter.pass(
            "godot:global",
            "engine",
            "get-max-physics-steps-per-frame"
        ))?;
        Ok(Engine::singleton().get_max_physics_steps_per_frame())
    }

    fn set_max_physics_steps_per_frame(&mut self, v: i32) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass(
            "godot:global",
            "engine",
            "set-max-physics-steps-per-frame"
        ))?;
        Engine::singleton().set_max_physics_steps_per_frame(v);
        Ok(())
    }

    fn get_physics_jitter_fix(&mut self) -> AnyResult<f64> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-physics-jitter-fix"))?;
        Ok(Engine::singleton().get_physics_jitter_fix())
    }

    fn set_physics_jitter_fix(&mut self, v: f64) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "set-physics-jitter-fix"))?;
        Engine::singleton().set_physics_jitter_fix(v);
        Ok(())
    }

    fn get_physics_ticks_per_second(&mut self) -> AnyResult<i32> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-physics-ticks-per-second"))?;
        Ok(Engine::singleton().get_physics_ticks_per_second())
    }

    fn set_physics_ticks_per_second(&mut self, v: i32) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "set-physics-ticks-per-second"))?;
        Engine::singleton().set_physics_ticks_per_second(v);
        Ok(())
    }

    fn is_printing_error_messages(&mut self) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "is-printing-error-messages"))?;
        Ok(Engine::singleton().is_printing_error_messages())
    }

    fn set_print_error_messages(&mut self, v: bool) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "set-print-error-messages"))?;
        Engine::singleton().set_print_error_messages(v);
        Ok(())
    }

    fn get_time_scale(&mut self) -> AnyResult<f64> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "get-time-scale"))?;
        Ok(Engine::singleton().get_time_scale())
    }

    fn set_time_scale(&mut self, v: f64) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "set-time-scale"))?;
        Engine::singleton().set_time_scale(v);
        Ok(())
    }

    fn get_architecture_name(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-architecture-name"))?;
        this.set_into_var(Engine::singleton().get_architecture_name())
    }

    fn get_author_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-author-info"))?;
        this.set_into_var(Engine::singleton().get_author_info())
    }

    fn get_copyright_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-copyright-info"))?;
        this.set_into_var(Engine::singleton().get_copyright_info())
    }

    fn get_donor_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "get-donor-info"))?;
        this.set_into_var(Engine::singleton().get_donor_info())
    }

    fn get_frames_drawn(&mut self) -> AnyResult<i32> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-frames-drawn"))?;
        Ok(Engine::singleton().get_frames_drawn())
    }

    fn get_frames_per_second(&mut self) -> AnyResult<f64> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-frames-per-second"))?;
        Ok(Engine::singleton().get_frames_per_second())
    }

    fn get_license_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-license-info"))?;
        this.set_into_var(Engine::singleton().get_license_info())
    }

    fn get_license_text(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-license-text"))?;
        this.set_into_var(Engine::singleton().get_license_text())
    }

    fn get_main_loop(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "get-main-loop"))?;
        this.set_into_var(Engine::singleton().get_main_loop())
    }

    fn get_physics_frames(&mut self) -> AnyResult<u64> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-physics-frames"))?;
        Ok(Engine::singleton().get_physics_frames())
    }

    fn get_physics_interpolation_fraction(&mut self) -> AnyResult<f64> {
        let this = self.as_mut();
        site_context!(this.filter.pass(
            "godot:global",
            "engine",
            "get-physics-interpolation-fraction"
        ))?;
        Ok(Engine::singleton().get_physics_interpolation_fraction())
    }

    fn get_process_frames(&mut self) -> AnyResult<u64> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-process-frames"))?;
        Ok(Engine::singleton().get_process_frames())
    }

    fn get_script_language(&mut self, ix: i32) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-script-language"))?;
        Engine::singleton()
            .get_script_language(ix)
            .map(|v| this.set_into_var(v))
            .transpose()
    }

    fn get_script_language_count(&mut self) -> AnyResult<i32> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-script-language-count"))?;
        Ok(Engine::singleton().get_script_language_count())
    }

    fn get_singleton(
        &mut self,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "get-singleton"))?;
        let name: StringName = this.get_value(name)?;
        Engine::singleton()
            .get_singleton(name)
            .map(|v| this.set_into_var(v))
            .transpose()
    }

    fn get_singleton_list(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-singleton-list"))?;
        this.set_into_var(Engine::singleton().get_singleton_list())
    }

    fn get_version_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-version-info"))?;
        this.set_into_var(Engine::singleton().get_version_info())
    }

    fn get_write_movie_path(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "get-write-movie-path"))?;
        this.set_into_var(Engine::singleton().get_write_movie_path())
    }

    fn has_singleton(&mut self, name: WasmResource<Variant>) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "has-singleton"))?;
        Ok(Engine::singleton().has_singleton(this.get_value(name)?))
    }

    fn is_editor_hint(&mut self) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:global", "engine", "is-editor-hint"))?;
        Ok(Engine::singleton().is_editor_hint())
    }

    fn is_in_physics_frame(&mut self) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "is-in-physics-frame"))?;
        Ok(Engine::singleton().is_in_physics_frame())
    }

    fn register_script_language(&mut self, lang: WasmResource<Variant>) -> ErrorRes {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "register-script-language"))?;
        wrap_error(Engine::singleton().register_script_language(this.get_value(lang)?))
    }

    fn unregister_script_language(&mut self, lang: WasmResource<Variant>) -> ErrorRes {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "unregister-script-language"))?;
        wrap_error(Engine::singleton().register_script_language(this.get_value(lang)?))
    }

    fn register_singleton(
        &mut self,
        name: WasmResource<Variant>,
        inst: WasmResource<Variant>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "register-singleton"))?;
        Engine::singleton().register_singleton(this.get_value(name)?, this.get_value(inst)?);
        Ok(())
    }

    fn unregister_singleton(&mut self, name: WasmResource<Variant>) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this
            .filter
            .pass("godot:global", "engine", "unregister-singleton"))?;
        Engine::singleton().unregister_singleton(this.get_value(name)?);
        Ok(())
    }
}

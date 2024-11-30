use anyhow::Result as AnyResult;
use godot::classes::{Engine, ScriptLanguage};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;
use crate::godot_component::{ErrorRes, GodotCtx, bindgen, wrap_error};

filter_macro! {method [
    singleton -> "singleton",
    get_max_fps -> "get-max-fps",
    get_max_physics_steps_per_frame -> "get-max-physics-steps-per-frame",
    get_physics_jitter_fix -> "get-physics-jitter-fix",
    get_physics_ticks_per_second -> "get-physics-ticks-per-second",
    is_printing_error_messages -> "is-printing-error-messages",
    get_time_scale -> "get-time-scale",
    set_max_fps -> "set-max-fps",
    set_max_physics_steps_per_frame -> "set-max-physics-steps-per-frame",
    set_physics_jitter_fix -> "set-physics-jitter-fix",
    set_physics_ticks_per_second -> "set-physics-ticks-per-second",
    set_print_error_messages -> "set-print-error-messages",
    set_time_scale -> "set-time-scale",
    get_architecture_name -> "get-architecture-name",
    get_author_info -> "get-author-info",
    get_copyright_info -> "get-copyright-info",
    get_donor_info -> "get-donor-info",
    get_license_info -> "get-license-info",
    get_license_text -> "get-license-text",
    get_version_info -> "get-version-info",
    get_frames_drawn -> "get-frames-drawn",
    get_frames_per_second -> "get-frames-per-second",
    get_physics_frames -> "get-physics-frames",
    get_process_frames -> "get-process-frames",
    get_physics_interpolation_fraction -> "get-physics-interpolation-fraction",
    is_in_physics_frame -> "is-in-physics-frame",
    get_main_loop -> "get-main-loop",
    is_editor_hint -> "is-editor-hint",
    has_singleton -> "has-singleton",
    get_singleton -> "get-singleton",
    get_singleton_list -> "get-singleton-list",
    register_singleton -> "register-singleton",
    unregister_singleton -> "unregister-singleton",
    get_script_language -> "get-script-language",
    get_script_language_count -> "get-script-language-count",
    register_script_language -> "register-script-language",
    unregister_script_language -> "unregister-script-language",
    get_write_movie_path -> "get-write-movie-path",
]}

impl bindgen::godot::global::engine::Host for GodotCtx {
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, singleton)?;
        self.set_into_var(Engine::singleton())
    }

    fn get_max_fps(&mut self) -> AnyResult<i32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_max_fps)?;
        Ok(Engine::singleton().get_max_fps())
    }

    fn set_max_fps(&mut self, v: i32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, set_max_fps)?;
        Engine::singleton().set_max_fps(v);
        Ok(())
    }

    fn get_max_physics_steps_per_frame(&mut self) -> AnyResult<i32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_max_physics_steps_per_frame)?;
        Ok(Engine::singleton().get_max_physics_steps_per_frame())
    }

    fn set_max_physics_steps_per_frame(&mut self, v: i32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, set_max_physics_steps_per_frame)?;
        Engine::singleton().set_max_physics_steps_per_frame(v);
        Ok(())
    }

    fn get_physics_jitter_fix(&mut self) -> AnyResult<f64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_physics_jitter_fix)?;
        Ok(Engine::singleton().get_physics_jitter_fix())
    }

    fn set_physics_jitter_fix(&mut self, v: f64) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, set_physics_jitter_fix)?;
        Engine::singleton().set_physics_jitter_fix(v);
        Ok(())
    }

    fn get_physics_ticks_per_second(&mut self) -> AnyResult<i32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_physics_ticks_per_second)?;
        Ok(Engine::singleton().get_physics_ticks_per_second())
    }

    fn set_physics_ticks_per_second(&mut self, v: i32) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, set_physics_ticks_per_second)?;
        Engine::singleton().set_physics_ticks_per_second(v);
        Ok(())
    }

    fn is_printing_error_messages(&mut self) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, is_printing_error_messages)?;
        Ok(Engine::singleton().is_printing_error_messages())
    }

    fn set_print_error_messages(&mut self, v: bool) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, set_print_error_messages)?;
        Engine::singleton().set_print_error_messages(v);
        Ok(())
    }

    fn get_time_scale(&mut self) -> AnyResult<f64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_time_scale)?;
        Ok(Engine::singleton().get_time_scale())
    }

    fn set_time_scale(&mut self, v: f64) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, set_time_scale)?;
        Engine::singleton().set_time_scale(v);
        Ok(())
    }

    fn get_architecture_name(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_architecture_name)?;
        self.set_into_var(Engine::singleton().get_architecture_name())
    }

    fn get_author_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_author_info)?;
        self.set_into_var(Engine::singleton().get_author_info())
    }

    fn get_copyright_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_copyright_info)?;
        self.set_into_var(Engine::singleton().get_copyright_info())
    }

    fn get_donor_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_donor_info)?;
        self.set_into_var(Engine::singleton().get_donor_info())
    }

    fn get_frames_drawn(&mut self) -> AnyResult<i32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_frames_drawn)?;
        Ok(Engine::singleton().get_frames_drawn())
    }

    fn get_frames_per_second(&mut self) -> AnyResult<f64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_frames_per_second)?;
        Ok(Engine::singleton().get_frames_per_second())
    }

    fn get_license_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_license_info)?;
        self.set_into_var(Engine::singleton().get_license_info())
    }

    fn get_license_text(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_license_info)?;
        self.set_into_var(Engine::singleton().get_license_text())
    }

    fn get_main_loop(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_main_loop)?;
        self.set_into_var(Engine::singleton().get_main_loop())
    }

    fn get_physics_frames(&mut self) -> AnyResult<u64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_physics_frames)?;
        Ok(Engine::singleton().get_physics_frames())
    }

    fn get_physics_interpolation_fraction(&mut self) -> AnyResult<f64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_physics_interpolation_fraction)?;
        Ok(Engine::singleton().get_physics_interpolation_fraction())
    }

    fn get_process_frames(&mut self) -> AnyResult<u64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_process_frames)?;
        Ok(Engine::singleton().get_process_frames())
    }

    fn get_script_language(&mut self, ix: i32) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_script_language)?;
        Engine::singleton()
            .get_script_language(ix)
            .map(|v| self.set_into_var(v))
            .transpose()
    }

    fn get_script_language_count(&mut self) -> AnyResult<i32> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_script_language_count)?;
        Ok(Engine::singleton().get_script_language_count())
    }

    fn get_singleton(
        &mut self,
        name: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_singleton)?;
        let name: StringName = self.get_value(name)?;
        self.set_var(Engine::singleton().get_singleton(&name).to_variant())
    }

    fn get_singleton_list(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_singleton_list)?;
        self.set_into_var(Engine::singleton().get_singleton_list())
    }

    fn get_version_info(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_version_info)?;
        self.set_into_var(Engine::singleton().get_version_info())
    }

    fn get_write_movie_path(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, get_write_movie_path)?;
        self.set_into_var(Engine::singleton().get_write_movie_path())
    }

    fn has_singleton(&mut self, name: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, has_singleton)?;
        Ok(Engine::singleton().has_singleton(&self.get_value::<StringName>(name)?))
    }

    fn is_editor_hint(&mut self) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, is_editor_hint)?;
        Ok(Engine::singleton().is_editor_hint())
    }

    fn is_in_physics_frame(&mut self) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, is_in_physics_frame)?;
        Ok(Engine::singleton().is_in_physics_frame())
    }

    fn register_script_language(&mut self, lang: WasmResource<Variant>) -> ErrorRes {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, register_script_language)?;
        wrap_error(
            Engine::singleton().register_script_language(&self.get_object::<ScriptLanguage>(lang)?),
        )
    }

    fn unregister_script_language(&mut self, lang: WasmResource<Variant>) -> ErrorRes {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, unregister_script_language)?;
        wrap_error(
            Engine::singleton()
                .unregister_script_language(&self.get_object::<ScriptLanguage>(lang)?),
        )
    }

    fn register_singleton(
        &mut self,
        name: WasmResource<Variant>,
        inst: WasmResource<Variant>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, register_singleton)?;
        Engine::singleton().register_singleton(
            &self.get_value::<StringName>(name)?,
            &self.get_object::<Object>(inst)?,
        );
        Ok(())
    }

    fn unregister_singleton(&mut self, name: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_global, engine, unregister_singleton)?;
        Engine::singleton().unregister_singleton(&self.get_value::<StringName>(name)?);
        Ok(())
    }
}

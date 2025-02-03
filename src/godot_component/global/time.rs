use anyhow::Result as AnyResult;
use godot::classes::Time;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;

filter_macro! {method [
    singleton -> "singleton",
    get_date_dict_from_system -> "get-date-dict-from-system",
    get_date_dict_from_unix_time -> "get-date-dict-from-unix-time",
    get_date_string_from_system -> "get-date-string-from-system",
    get_date_string_from_unix_time -> "get-date-string-from-unix-time",
    get_datetime_dict_from_datetime_string -> "get-datetime-dict-from-datetime-string",
    get_datetime_dict_from_system -> "get-datetime-dict-from-system",
    get_datetime_dict_from_unix_time -> "get-datetime-dict-from-unix-time",
    get_datetime_string_from_datetime_dict -> "get-datetime-string-from-datetime-dict",
    get_datetime_string_from_system -> "get-datetime-string-from-system",
    get_datetime_string_from_unix_time -> "get-datetime-string-from-unix-time",
    get_offset_string_from_offset_minutes -> "get-offset-string-from-offset-minutes",
    get_ticks_msec -> "get-ticks-msec",
    get_ticks_usec -> "get-ticks-usec",
    get_time_dict_from_system -> "get-time-dict-from-system",
    get_time_dict_from_unix_time -> "get-time-dict-from-unix-time",
    get_time_string_from_system -> "get-time-string-from-system",
    get_time_string_from_unix_time -> "get-time-string-from-unix-time",
    get_time_zone_from_system -> "get-time-zone-from-system",
    get_unix_time_from_datetime_dict -> "get-unix-time-from-datetime-dict",
    get_unix_time_from_datetime_string -> "get-unix-time-from-datetime-string",
    get_unix_time_from_system -> "get-unix-time-from-system",
]}

impl crate::godot_component::bindgen::godot::global::time::Host
    for crate::godot_component::GodotCtx
{
    fn singleton(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, singleton)?;
        self.set_into_var(Time::singleton())
    }

    fn get_date_dict_from_system(&mut self, utc: bool) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_date_dict_from_system)?;
        self.set_into_var(
            Time::singleton()
                .get_date_dict_from_system_ex()
                .utc(utc)
                .done(),
        )
    }

    fn get_date_dict_from_unix_time(&mut self, time: i64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_date_dict_from_unix_time)?;
        self.set_into_var(Time::singleton().get_date_dict_from_unix_time(time))
    }

    fn get_date_string_from_system(&mut self, utc: bool) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_date_string_from_system)?;
        self.set_into_var(
            Time::singleton()
                .get_date_string_from_system_ex()
                .utc(utc)
                .done(),
        )
    }

    fn get_date_string_from_unix_time(&mut self, time: i64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_date_string_from_unix_time)?;
        self.set_into_var(Time::singleton().get_date_string_from_unix_time(time))
    }

    fn get_datetime_dict_from_datetime_string(
        &mut self,
        s: WasmResource<Variant>,
        weekday: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_datetime_dict_from_datetime_string)?;
        let s: GString = self.get_value(s)?;
        self.set_into_var(Time::singleton().get_datetime_dict_from_datetime_string(&s, weekday))
    }

    fn get_datetime_dict_from_system(&mut self, utc: bool) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_datetime_dict_from_system)?;
        self.set_into_var(
            Time::singleton()
                .get_datetime_dict_from_system_ex()
                .utc(utc)
                .done(),
        )
    }

    fn get_datetime_dict_from_unix_time(&mut self, time: i64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_datetime_dict_from_unix_time)?;
        self.set_into_var(Time::singleton().get_datetime_dict_from_unix_time(time))
    }

    fn get_datetime_string_from_datetime_dict(
        &mut self,
        d: WasmResource<Variant>,
        space: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_datetime_string_from_datetime_dict)?;
        let d: Dictionary = self.get_value(d)?;
        self.set_into_var(Time::singleton().get_datetime_string_from_datetime_dict(&d, space))
    }

    fn get_datetime_string_from_system(
        &mut self,
        utc: bool,
        space: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_datetime_string_from_system)?;
        self.set_into_var(
            Time::singleton()
                .get_datetime_string_from_system_ex()
                .utc(utc)
                .use_space(space)
                .done(),
        )
    }

    fn get_datetime_string_from_unix_time(
        &mut self,
        time: i64,
        space: bool,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_datetime_string_from_unix_time)?;
        self.set_into_var(
            Time::singleton()
                .get_datetime_string_from_unix_time_ex(time)
                .use_space(space)
                .done(),
        )
    }

    fn get_offset_string_from_offset_minutes(
        &mut self,
        offset: i64,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_offset_string_from_offset_minutes)?;
        self.set_into_var(Time::singleton().get_offset_string_from_offset_minutes(offset))
    }

    fn get_ticks_msec(&mut self) -> AnyResult<u64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_ticks_msec)?;
        Ok(Time::singleton().get_ticks_msec())
    }

    fn get_ticks_usec(&mut self) -> AnyResult<u64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_ticks_usec)?;
        Ok(Time::singleton().get_ticks_usec())
    }

    fn get_time_dict_from_system(&mut self, utc: bool) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_time_dict_from_system)?;
        self.set_into_var(
            Time::singleton()
                .get_time_dict_from_system_ex()
                .utc(utc)
                .done(),
        )
    }

    fn get_time_dict_from_unix_time(&mut self, time: i64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_time_dict_from_unix_time)?;
        self.set_into_var(Time::singleton().get_time_dict_from_unix_time(time))
    }

    fn get_time_string_from_system(&mut self, utc: bool) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_time_string_from_system)?;
        self.set_into_var(
            Time::singleton()
                .get_time_string_from_system_ex()
                .utc(utc)
                .done(),
        )
    }

    fn get_time_string_from_unix_time(&mut self, time: i64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_time_string_from_unix_time)?;
        self.set_into_var(Time::singleton().get_time_string_from_unix_time(time))
    }

    fn get_time_zone_from_system(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_time_zone_from_system)?;
        self.set_into_var(Time::singleton().get_time_zone_from_system())
    }

    fn get_unix_time_from_datetime_dict(&mut self, val: WasmResource<Variant>) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_unix_time_from_datetime_dict)?;
        Ok(Time::singleton().get_unix_time_from_datetime_dict(&self.get_value(val)?))
    }

    fn get_unix_time_from_datetime_string(&mut self, val: WasmResource<Variant>) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_unix_time_from_datetime_string)?;
        Ok(Time::singleton().get_unix_time_from_datetime_string(&self.get_value::<GString>(val)?))
    }

    fn get_unix_time_from_system(&mut self) -> AnyResult<f64> {
        filter_macro!(filter self.filter.as_ref(), godot_global, time, get_unix_time_from_system)?;
        Ok(Time::singleton().get_unix_time_from_system())
    }
}

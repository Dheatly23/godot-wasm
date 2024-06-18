use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;

filter_macro! {method [
    empty -> "empty",
    len -> "len",
    is_empty -> "is-empty",
    clear -> "clear",
    duplicate -> "duplicate",
    get -> "get",
    has -> "has",
    has_all -> "has-all",
    insert -> "insert",
    remove -> "remove",
    extend -> "extend",
    keys -> "keys",
    values -> "values",
    extend_list -> "extend-list",
    from_list -> "from-list",
    into_list -> "into-list",
]}

impl crate::godot_component::bindgen::godot::core::dictionary::Host
    for crate::godot_component::GodotCtx
{
    fn empty(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, empty)?;
        self.set_into_var(Dictionary::new())
    }

    fn from_list(
        &mut self,
        val: Vec<(Option<WasmResource<Variant>>, Option<WasmResource<Variant>>)>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, from_list)?;
        let v = val
            .into_iter()
            .map(|(k, v)| Ok((self.maybe_get_var(k)?, self.maybe_get_var(v)?)))
            .collect::<AnyResult<Dictionary>>()?;
        self.set_into_var(v)
    }

    fn into_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Vec<(Option<WasmResource<Variant>>, Option<WasmResource<Variant>>)>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, into_list)?;
        let v: Dictionary = self.get_value(var)?;
        v.iter_shared()
            .map(|(k, v)| Ok((self.set_var(k)?, self.set_var(v)?)))
            .collect()
    }

    fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, len)?;
        Ok(self.get_value::<Dictionary>(var)?.len() as _)
    }

    fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, is_empty)?;
        Ok(self.get_value::<Dictionary>(var)?.is_empty())
    }

    fn clear(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, clear)?;
        self.get_value::<Dictionary>(var)?.clear();
        Ok(())
    }

    fn duplicate(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, duplicate)?;
        let r = self.get_value::<Dictionary>(var)?.duplicate_shallow();
        self.set_into_var(r)
    }

    fn get(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<Option<WasmResource<Variant>>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, get)?;
        let v: Dictionary = self.get_value(var)?;
        match v.get(self.maybe_get_var(key)?) {
            Some(v) => self.set_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn has(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, has)?;
        Ok(self
            .get_value::<Dictionary>(var)?
            .contains_key(self.maybe_get_var(key)?))
    }

    fn has_all(
        &mut self,
        var: WasmResource<Variant>,
        key: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, has_all)?;
        Ok(self
            .get_value::<Dictionary>(var)?
            .contains_all_keys(self.get_value(key)?))
    }

    fn insert(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<Option<WasmResource<Variant>>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, insert)?;
        let mut v: Dictionary = self.get_value(var)?;
        match v.insert(self.maybe_get_var(key)?, self.maybe_get_var(val)?) {
            Some(v) => self.set_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn remove(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<Option<WasmResource<Variant>>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, remove)?;
        let mut v: Dictionary = self.get_value(var)?;
        match v.remove(self.maybe_get_var(key)?) {
            Some(v) => self.set_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn extend(
        &mut self,
        var: WasmResource<Variant>,
        other: WasmResource<Variant>,
        overwrite: bool,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, extend)?;
        let mut v: Dictionary = self.get_value(var)?;
        v.extend_dictionary(self.get_value(other)?, overwrite);
        Ok(())
    }

    fn keys(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, keys)?;
        let v: Dictionary = self.get_value(var)?;
        self.set_into_var(v.keys_array())
    }

    fn values(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, values)?;
        let v: Dictionary = self.get_value(var)?;
        self.set_into_var(v.values_array())
    }

    fn extend_list(
        &mut self,
        var: WasmResource<Variant>,
        val: Vec<(Option<WasmResource<Variant>>, Option<WasmResource<Variant>>)>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, dictionary, extend_list)?;
        let mut var: Dictionary = self.get_value(var)?;

        for (k, v) in val.into_iter() {
            var.set(self.maybe_get_var(k)?, self.maybe_get_var(v)?);
        }

        Ok(())
    }
}

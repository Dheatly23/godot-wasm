use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

impl crate::godot_component::bindgen::godot::core::dictionary::Host
    for crate::godot_component::GodotCtx
{
    fn empty(&mut self) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&Dictionary::new()))
    }

    fn from_list(
        &mut self,
        val: Vec<(Option<WasmResource<Variant>>, Option<WasmResource<Variant>>)>,
    ) -> AnyResult<WasmResource<Variant>> {
        let v = val
            .into_iter()
            .map(|(k, v)| Ok((self.maybe_get_var(k)?, self.maybe_get_var(v)?)))
            .collect::<AnyResult<Dictionary>>()?;
        Ok(self.set_into_var(&v))
    }

    fn into_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Vec<(Option<WasmResource<Variant>>, Option<WasmResource<Variant>>)>> {
        let v: Dictionary = self.get_var_borrow(var)?.try_to()?;
        Ok(v.iter_shared()
            .map(|(k, v)| (self.set_var(k), self.set_var(v)))
            .collect())
    }

    fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
        Ok(self.get_var_borrow(var)?.try_to::<Dictionary>()?.len() as _)
    }

    fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.try_to::<Dictionary>()?.is_empty())
    }

    fn clear(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        self.get_var_borrow(var)?.try_to::<Dictionary>()?.clear();
        Ok(())
    }

    fn duplicate(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let r = self
            .get_var_borrow(var)?
            .try_to::<Dictionary>()?
            .duplicate_shallow();
        Ok(self.set_into_var(&r))
    }

    fn get(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<Option<WasmResource<Variant>>>> {
        let v: Dictionary = self.get_var_borrow(var)?.try_to()?;
        Ok(v.get(self.maybe_get_var(key)?).map(|v| self.set_var(v)))
    }

    fn has(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
    ) -> AnyResult<bool> {
        Ok(self
            .get_var_borrow(var)?
            .try_to::<Dictionary>()?
            .contains_key(self.maybe_get_var(key)?))
    }

    fn has_all(
        &mut self,
        var: WasmResource<Variant>,
        key: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        Ok(self
            .get_var_borrow(var)?
            .try_to::<Dictionary>()?
            .contains_all_keys(self.get_var_borrow(key)?.try_to()?))
    }

    fn insert(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<Option<WasmResource<Variant>>>> {
        let mut v: Dictionary = self.get_var_borrow(var)?.try_to()?;
        Ok(v.insert(self.maybe_get_var(key)?, self.maybe_get_var(val)?)
            .map(|v| self.set_var(v)))
    }

    fn remove(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<Option<WasmResource<Variant>>>> {
        let mut v: Dictionary = self.get_var_borrow(var)?.try_to()?;
        Ok(v.remove(self.maybe_get_var(key)?).map(|v| self.set_var(v)))
    }

    fn extend(
        &mut self,
        var: WasmResource<Variant>,
        other: WasmResource<Variant>,
        overwrite: bool,
    ) -> AnyResult<()> {
        let mut v: Dictionary = self.get_var_borrow(var)?.try_to()?;
        v.extend_dictionary(self.get_var_borrow(other)?.try_to()?, overwrite);
        Ok(())
    }

    fn keys(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let v: Dictionary = self.get_var_borrow(var)?.try_to()?;
        Ok(self.set_into_var(&v.keys_array()))
    }

    fn values(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let v: Dictionary = self.get_var_borrow(var)?.try_to()?;
        Ok(self.set_into_var(&v.values_array()))
    }

    fn extend_list(
        &mut self,
        var: WasmResource<Variant>,
        val: Vec<(Option<WasmResource<Variant>>, Option<WasmResource<Variant>>)>,
    ) -> AnyResult<()> {
        let mut var: Dictionary = self.get_var_borrow(var)?.try_to()?;

        for (k, v) in val.into_iter() {
            var.insert(self.maybe_get_var(k)?, self.maybe_get_var(v)?);
        }

        Ok(())
    }
}

use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

impl<T: AsMut<crate::godot_component::GodotCtx>>
    crate::godot_component::bindgen::godot::core::dictionary::Host for T
{
    fn empty(&mut self) -> AnyResult<WasmResource<Variant>> {
        self.as_mut().set_into_var(Dictionary::new())
    }

    fn from_list(
        &mut self,
        val: Vec<(Option<WasmResource<Variant>>, Option<WasmResource<Variant>>)>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let v = val
            .into_iter()
            .map(|(k, v)| Ok((this.maybe_get_var(k)?, this.maybe_get_var(v)?)))
            .collect::<AnyResult<Dictionary>>()?;
        this.set_into_var(v)
    }

    fn into_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Vec<(Option<WasmResource<Variant>>, Option<WasmResource<Variant>>)>> {
        let this = self.as_mut();
        let v: Dictionary = this.get_var_borrow(var)?.try_to()?;
        v.iter_shared()
            .map(|(k, v)| Ok((this.set_var(k)?, this.set_var(v)?)))
            .collect()
    }

    fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
        Ok(self
            .as_mut()
            .get_var_borrow(var)?
            .try_to::<Dictionary>()?
            .len() as _)
    }

    fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self
            .as_mut()
            .get_var_borrow(var)?
            .try_to::<Dictionary>()?
            .is_empty())
    }

    fn clear(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        self.as_mut()
            .get_var_borrow(var)?
            .try_to::<Dictionary>()?
            .clear();
        Ok(())
    }

    fn duplicate(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let r = this
            .get_var_borrow(var)?
            .try_to::<Dictionary>()?
            .duplicate_shallow();
        this.set_into_var(r)
    }

    fn get(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<Option<WasmResource<Variant>>>> {
        let this = self.as_mut();
        let v: Dictionary = this.get_var_borrow(var)?.try_to()?;
        match v.get(this.maybe_get_var(key)?) {
            Some(v) => this.set_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn has(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        Ok(this
            .get_var_borrow(var)?
            .try_to::<Dictionary>()?
            .contains_key(this.maybe_get_var(key)?))
    }

    fn has_all(
        &mut self,
        var: WasmResource<Variant>,
        key: WasmResource<Variant>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        Ok(this
            .get_var_borrow(var)?
            .try_to::<Dictionary>()?
            .contains_all_keys(this.get_var_borrow(key)?.try_to()?))
    }

    fn insert(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
        val: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<Option<WasmResource<Variant>>>> {
        let this = self.as_mut();
        let mut v: Dictionary = this.get_var_borrow(var)?.try_to()?;
        match v.insert(this.maybe_get_var(key)?, this.maybe_get_var(val)?) {
            Some(v) => this.set_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn remove(
        &mut self,
        var: WasmResource<Variant>,
        key: Option<WasmResource<Variant>>,
    ) -> AnyResult<Option<Option<WasmResource<Variant>>>> {
        let this = self.as_mut();
        let mut v: Dictionary = this.get_var_borrow(var)?.try_to()?;
        match v.remove(this.maybe_get_var(key)?) {
            Some(v) => this.set_var(v).map(Some),
            None => Ok(None),
        }
    }

    fn extend(
        &mut self,
        var: WasmResource<Variant>,
        other: WasmResource<Variant>,
        overwrite: bool,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let mut v: Dictionary = this.get_var_borrow(var)?.try_to()?;
        v.extend_dictionary(this.get_var_borrow(other)?.try_to()?, overwrite);
        Ok(())
    }

    fn keys(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let v: Dictionary = this.get_var_borrow(var)?.try_to()?;
        this.set_into_var(v.keys_array())
    }

    fn values(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        let v: Dictionary = this.get_var_borrow(var)?.try_to()?;
        this.set_into_var(v.values_array())
    }

    fn extend_list(
        &mut self,
        var: WasmResource<Variant>,
        val: Vec<(Option<WasmResource<Variant>>, Option<WasmResource<Variant>>)>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        let mut var: Dictionary = this.get_var_borrow(var)?.try_to()?;

        for (k, v) in val.into_iter() {
            var.insert(this.maybe_get_var(k)?, this.maybe_get_var(v)?);
        }

        Ok(())
    }
}

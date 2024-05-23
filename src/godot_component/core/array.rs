use anyhow::{bail, Result as AnyResult};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;

filter_macro! {method [
    empty -> "empty",
    from_list -> "from-list",
    to_list -> "to-list",
    len -> "len",
    is_empty -> "is-empty",
    resize -> "resize",
    shrink -> "shrink",
    clear -> "clear",
    reverse -> "reverse",
    duplicate -> "duplicate",
    subarray -> "subarray",
    get -> "get",
    set -> "set",
    extend -> "extend",
    push_back -> "push-back",
    push_front -> "push-front",
    pop_back -> "pop-back",
    pop_front -> "pop-front",
    insert -> "insert",
    remove -> "remove",
    erase -> "erase",
    fill -> "fill",
    contains -> "rfind",
    count -> "find",
    find -> "count",
    rfind -> "contains",
]}

impl crate::godot_component::bindgen::godot::core::array::Host
    for crate::godot_component::GodotCtx
{
    fn empty(&mut self) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, empty)?;
        self.set_into_var(VariantArray::new())
    }

    fn from_list(
        &mut self,
        val: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, from_list)?;
        let v: VariantArray = val
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<_>>()?;
        self.set_into_var(v)
    }

    fn to_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Vec<Option<WasmResource<Variant>>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, to_list)?;
        let v: VariantArray = self.get_value(var)?;
        v.iter_shared().map(|v| self.set_var(v)).collect()
    }

    fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, len)?;
        let v: VariantArray = self.get_value(var)?;
        Ok(v.len() as _)
    }

    fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, is_empty)?;
        let v: VariantArray = self.get_value(var)?;
        Ok(v.is_empty())
    }

    fn resize(
        &mut self,
        var: WasmResource<Variant>,
        n: u32,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, resize)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.resize(n as _, &*self.maybe_get_var_borrow(item)?);
        Ok(())
    }

    fn shrink(&mut self, var: WasmResource<Variant>, n: u32) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, shrink)?;
        let mut v: VariantArray = self.get_value(var)?;
        Ok(v.shrink(n as _))
    }

    fn clear(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, clear)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.clear();
        Ok(())
    }

    fn reverse(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, reverse)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.reverse();
        Ok(())
    }

    fn duplicate(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, duplicate)?;
        let v: VariantArray = self.get_value(var)?;
        self.set_into_var(v.duplicate_shallow())
    }

    fn subarray(
        &mut self,
        var: WasmResource<Variant>,
        begin: u32,
        end: u32,
        step: Option<u32>,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, subarray)?;
        let v: VariantArray = self.get_value(var)?;
        self.set_into_var(v.subarray_shallow(begin as _, end as _, step.map(|v| v as _)))
    }

    fn get(
        &mut self,
        var: WasmResource<Variant>,
        ix: u32,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, get)?;
        let v: VariantArray = self.get_value(var)?;
        let Some(r) = v.try_get(ix as _) else {
            bail!("index {ix} out of bound")
        };
        self.set_var(r)
    }

    fn set(
        &mut self,
        var: WasmResource<Variant>,
        ix: u32,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, set)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.set(ix as _, self.maybe_get_var(item)?);
        Ok(())
    }

    fn extend(
        &mut self,
        var: WasmResource<Variant>,
        other: WasmResource<Variant>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, extend)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.extend_array(self.get_value(other)?);
        Ok(())
    }

    fn push_back(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, push_back)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.push(self.maybe_get_var(item)?);
        Ok(())
    }

    fn push_front(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, push_front)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.push_front(self.maybe_get_var(item)?);
        Ok(())
    }

    fn pop_back(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, pop_back)?;
        let mut v: VariantArray = self.get_value(var)?;
        match v.pop() {
            Some(v) => self.set_var(v),
            None => Ok(None),
        }
    }

    fn pop_front(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, pop_front)?;
        let mut v: VariantArray = self.get_value(var)?;
        match v.pop_front() {
            Some(v) => self.set_var(v),
            None => Ok(None),
        }
    }

    fn insert(
        &mut self,
        var: WasmResource<Variant>,
        i: u32,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, insert)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.insert(i as _, self.maybe_get_var(item)?);
        Ok(())
    }

    fn remove(
        &mut self,
        var: WasmResource<Variant>,
        i: u32,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, remove)?;
        let mut v: VariantArray = self.get_value(var)?;
        self.set_var(v.remove(i as _))
    }

    fn erase(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, erase)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.erase(&*self.maybe_get_var_borrow(item)?);
        Ok(())
    }

    fn fill(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, fill)?;
        let mut v: VariantArray = self.get_value(var)?;
        v.fill(&*self.maybe_get_var_borrow(item)?);
        Ok(())
    }

    fn contains(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, contains)?;
        let v: VariantArray = self.get_value(var)?;
        Ok(v.contains(&*self.maybe_get_var_borrow(item)?))
    }

    fn count(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<u32> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, count)?;
        let v: VariantArray = self.get_value(var)?;
        Ok(v.count(&*self.maybe_get_var_borrow(item)?) as _)
    }

    fn find(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
        from: Option<u32>,
    ) -> AnyResult<Option<u32>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, find)?;
        let v: VariantArray = self.get_value(var)?;
        let i = self.maybe_get_var_borrow(item)?;
        Ok(v.find(&*i, from.map(|v| v as _)).map(|v| v as _))
    }

    fn rfind(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
        from: Option<u32>,
    ) -> AnyResult<Option<u32>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, array, rfind)?;
        let v: VariantArray = self.get_value(var)?;
        let i = self.maybe_get_var_borrow(item)?;
        Ok(v.rfind(&*i, from.map(|v| v as _)).map(|v| v as _))
    }
}

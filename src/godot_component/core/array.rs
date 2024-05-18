use anyhow::{bail, Result as AnyResult};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::site_context;

impl<T: AsMut<crate::godot_component::GodotCtx>>
    crate::godot_component::bindgen::godot::core::array::Host for T
{
    fn empty(&mut self) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "empty"))?;
        this.set_into_var(VariantArray::new())
    }

    fn from_list(
        &mut self,
        val: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "from-list"))?;
        let v: VariantArray = val
            .into_iter()
            .map(|v| this.maybe_get_var(v))
            .collect::<AnyResult<_>>()?;
        this.set_into_var(v)
    }

    fn to_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Vec<Option<WasmResource<Variant>>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "to-list"))?;
        let v: VariantArray = this.get_value(var)?;
        v.iter_shared().map(|v| this.set_var(v)).collect()
    }

    fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "len"))?;
        let v: VariantArray = this.get_value(var)?;
        Ok(v.len() as _)
    }

    fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "is-empty"))?;
        let v: VariantArray = this.get_value(var)?;
        Ok(v.is_empty())
    }

    fn resize(
        &mut self,
        var: WasmResource<Variant>,
        n: u32,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "resize"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.resize(n as _, &*this.maybe_get_var_borrow(item)?);
        Ok(())
    }

    fn shrink(&mut self, var: WasmResource<Variant>, n: u32) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "shrink"))?;
        let mut v: VariantArray = this.get_value(var)?;
        Ok(v.shrink(n as _))
    }

    fn clear(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "clear"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.clear();
        Ok(())
    }

    fn reverse(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "reverse"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.reverse();
        Ok(())
    }

    fn duplicate(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "duplicate"))?;
        let v: VariantArray = this.get_value(var)?;
        this.set_into_var(v.duplicate_shallow())
    }

    fn subarray(
        &mut self,
        var: WasmResource<Variant>,
        begin: u32,
        end: u32,
        step: Option<u32>,
    ) -> AnyResult<WasmResource<Variant>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "subarray"))?;
        let v: VariantArray = this.get_value(var)?;
        this.set_into_var(v.subarray_shallow(begin as _, end as _, step.map(|v| v as _)))
    }

    fn get(
        &mut self,
        var: WasmResource<Variant>,
        ix: u32,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "get"))?;
        let v: VariantArray = this.get_value(var)?;
        let Some(r) = v.try_get(ix as _) else {
            bail!("index {ix} out of bound")
        };
        this.set_var(r)
    }

    fn set(
        &mut self,
        var: WasmResource<Variant>,
        ix: u32,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "set"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.set(ix as _, this.maybe_get_var(item)?);
        Ok(())
    }

    fn extend(
        &mut self,
        var: WasmResource<Variant>,
        other: WasmResource<Variant>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "extend"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.extend_array(this.get_value(other)?);
        Ok(())
    }

    fn push_back(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "push-back"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.push(this.maybe_get_var(item)?);
        Ok(())
    }

    fn push_front(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "push-front"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.push_front(this.maybe_get_var(item)?);
        Ok(())
    }

    fn pop_back(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "pop-back"))?;
        let mut v: VariantArray = this.get_value(var)?;
        match v.pop() {
            Some(v) => this.set_var(v),
            None => Ok(None),
        }
    }

    fn pop_front(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "pop-front"))?;
        let mut v: VariantArray = this.get_value(var)?;
        match v.pop_front() {
            Some(v) => this.set_var(v),
            None => Ok(None),
        }
    }

    fn insert(
        &mut self,
        var: WasmResource<Variant>,
        i: u32,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "insert"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.insert(i as _, this.maybe_get_var(item)?);
        Ok(())
    }

    fn remove(
        &mut self,
        var: WasmResource<Variant>,
        i: u32,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "remove"))?;
        let mut v: VariantArray = this.get_value(var)?;
        this.set_var(v.remove(i as _))
    }

    fn erase(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "erase"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.erase(&*this.maybe_get_var_borrow(item)?);
        Ok(())
    }

    fn fill(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "fill"))?;
        let mut v: VariantArray = this.get_value(var)?;
        v.fill(&*this.maybe_get_var_borrow(item)?);
        Ok(())
    }

    fn contains(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<bool> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "contains"))?;
        let v: VariantArray = this.get_value(var)?;
        Ok(v.contains(&*this.maybe_get_var_borrow(item)?))
    }

    fn count(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<u32> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "count"))?;
        let v: VariantArray = this.get_value(var)?;
        Ok(v.count(&*this.maybe_get_var_borrow(item)?) as _)
    }

    fn find(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
        from: Option<u32>,
    ) -> AnyResult<Option<u32>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "find"))?;
        let v: VariantArray = this.get_value(var)?;
        let i = this.maybe_get_var_borrow(item)?;
        Ok(v.find(&*i, from.map(|v| v as _)).map(|v| v as _))
    }

    fn rfind(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
        from: Option<u32>,
    ) -> AnyResult<Option<u32>> {
        let this = self.as_mut();
        site_context!(this.filter.pass("godot:core", "array", "rfind"))?;
        let v: VariantArray = this.get_value(var)?;
        let i = this.maybe_get_var_borrow(item)?;
        Ok(v.rfind(&*i, from.map(|v| v as _)).map(|v| v as _))
    }
}

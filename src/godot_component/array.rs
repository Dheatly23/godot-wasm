use anyhow::{bail, Result as AnyResult};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

impl crate::godot_component::bindgen::godot::core::array::Host
    for crate::godot_component::GodotCtx
{
    fn empty(&mut self) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&<Array<Variant>>::new()))
    }

    fn from_list(
        &mut self,
        val: Vec<Option<WasmResource<Variant>>>,
    ) -> AnyResult<WasmResource<Variant>> {
        let v: Array<Variant> = val
            .into_iter()
            .map(|v| self.maybe_get_var(v))
            .collect::<AnyResult<_>>()?;
        Ok(self.set_into_var(&v))
    }

    fn to_list(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Vec<Option<WasmResource<Variant>>>> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(v.iter_shared().map(|v| self.set_var(v)).collect())
    }

    fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(v.len() as _)
    }

    fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(v.is_empty())
    }

    fn resize(
        &mut self,
        var: WasmResource<Variant>,
        n: u32,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.resize(n as _, &*self.maybe_get_var_borrow(item)?);
        Ok(())
    }

    fn shrink(&mut self, var: WasmResource<Variant>, n: u32) -> AnyResult<bool> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(v.shrink(n as _))
    }

    fn clear(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.clear();
        Ok(())
    }

    fn reverse(&mut self, var: WasmResource<Variant>) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.reverse();
        Ok(())
    }

    fn duplicate(&mut self, var: WasmResource<Variant>) -> AnyResult<WasmResource<Variant>> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(self.set_into_var(&v.duplicate_shallow()))
    }

    fn subarray(
        &mut self,
        var: WasmResource<Variant>,
        begin: u32,
        end: u32,
        step: Option<u32>,
    ) -> AnyResult<WasmResource<Variant>> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(self.set_into_var(&v.subarray_shallow(begin as _, end as _, step.map(|v| v as _))))
    }

    fn get(
        &mut self,
        var: WasmResource<Variant>,
        ix: u32,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        let Some(r) = v.try_get(ix as _) else {
            bail!("index {ix} out of bound")
        };
        Ok(self.set_var(r))
    }

    fn set(
        &mut self,
        var: WasmResource<Variant>,
        ix: u32,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.set(ix as _, self.maybe_get_var(item)?);
        Ok(())
    }

    fn extend(
        &mut self,
        var: WasmResource<Variant>,
        other: WasmResource<Variant>,
    ) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.extend_array(self.get_var_borrow(other)?.try_to()?);
        Ok(())
    }

    fn push_back(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.push(self.maybe_get_var(item)?);
        Ok(())
    }

    fn push_front(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.push_front(self.maybe_get_var(item)?);
        Ok(())
    }

    fn pop_back(&mut self, var: WasmResource<Variant>) -> AnyResult<Option<WasmResource<Variant>>> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(v.pop().and_then(|v| self.set_var(v)))
    }

    fn pop_front(
        &mut self,
        var: WasmResource<Variant>,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(v.pop_front().and_then(|v| self.set_var(v)))
    }

    fn insert(
        &mut self,
        var: WasmResource<Variant>,
        i: u32,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.insert(i as _, self.maybe_get_var(item)?);
        Ok(())
    }

    fn remove(
        &mut self,
        var: WasmResource<Variant>,
        i: u32,
    ) -> AnyResult<Option<WasmResource<Variant>>> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(self.set_var(v.remove(i as _)))
    }

    fn erase(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.erase(&*self.maybe_get_var_borrow(item)?);
        Ok(())
    }

    fn fill(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<()> {
        let mut v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        v.fill(&*self.maybe_get_var_borrow(item)?);
        Ok(())
    }

    fn contains(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<bool> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(v.contains(&*self.maybe_get_var_borrow(item)?))
    }

    fn count(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
    ) -> AnyResult<u32> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(v.count(&*self.maybe_get_var_borrow(item)?) as _)
    }

    fn find(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
        from: Option<u32>,
    ) -> AnyResult<Option<u32>> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(
            v.find(&*self.maybe_get_var_borrow(item)?, from.map(|v| v as _))
                .map(|v| v as _),
        )
    }

    fn rfind(
        &mut self,
        var: WasmResource<Variant>,
        item: Option<WasmResource<Variant>>,
        from: Option<u32>,
    ) -> AnyResult<Option<u32>> {
        let v: Array<Variant> = self.get_var_borrow(var)?.try_to()?;
        Ok(
            v.rfind(&*self.maybe_get_var_borrow(item)?, from.map(|v| v as _))
                .map(|v| v as _),
        )
    }
}

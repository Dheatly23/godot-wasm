use anyhow::{bail, Result as AnyResult};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::GodotCtx;

macro_rules! impl_packed_array {
    ($m:ident <$t:ty>) => {
        use crate::godot_component::bindgen::godot::core::$m;

        impl $m::Host for GodotCtx {
            fn from(&mut self, val: Vec<$m::Elem>) -> AnyResult<WasmResource<Variant>> {
                Ok(self.set_into_var(&<$t>::from(&*val)))
            }

            fn to(&mut self, var: WasmResource<Variant>) -> AnyResult<Vec<$m::Elem>> {
                Ok(self.get_var(var).try_to::<$t>()?.to_vec())
            }

            fn slice(
                &mut self,
                var: WasmResource<Variant>,
                begin: u32,
                end: u32,
            ) -> AnyResult<Vec<$m::Elem>> {
                let v: $t = self.get_var(var).try_to()?;
                let Some(v) = v.as_slice().get(begin as usize..end as usize) else {
                    bail!("index ({begin}..{end}) out of bound")
                };
                Ok(v.to_owned())
            }

            fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
                Ok(self.get_var(var).try_to::<$t>()?.len() as _)
            }

            fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
                Ok(self.get_var(var).try_to::<$t>()?.is_empty())
            }

            fn get(&mut self, var: WasmResource<Variant>, i: u32) -> AnyResult<$m::Elem> {
                let v: $t = self.get_var(var).try_to()?;
                let Some(v) = v.as_slice().get(i as usize) else {
                    bail!("index {i} out of bound")
                };
                Ok(*v)
            }

            fn contains(&mut self, var: WasmResource<Variant>, val: $m::Elem) -> AnyResult<bool> {
                Ok(self.get_var(var).try_to::<$t>()?.contains(val))
            }

            fn count(&mut self, var: WasmResource<Variant>, val: $m::Elem) -> AnyResult<u32> {
                Ok(self.get_var(var).try_to::<$t>()?.count(val) as _)
            }

            fn find(
                &mut self,
                var: WasmResource<Variant>,
                val: $m::Elem,
                from: Option<u32>,
            ) -> AnyResult<Option<u32>> {
                Ok(self
                    .get_var(var)
                    .try_to::<$t>()?
                    .find(val, from.map(|v| v as _))
                    .map(|v| v as _))
            }

            fn rfind(
                &mut self,
                var: WasmResource<Variant>,
                val: $m::Elem,
                from: Option<u32>,
            ) -> AnyResult<Option<u32>> {
                Ok(self
                    .get_var(var)
                    .try_to::<$t>()?
                    .rfind(val, from.map(|v| v as _))
                    .map(|v| v as _))
            }

            fn subarray(
                &mut self,
                var: WasmResource<Variant>,
                begin: u32,
                end: u32,
            ) -> AnyResult<WasmResource<Variant>> {
                let v: $t = self.get_var(var).try_to()?;
                Ok(self.set_into_var(&v.subarray(begin as _, end as _)))
            }
        }
    };
    ($m:ident <$t:ty> |$v:ident|($e1:expr, $e2:expr)) => {
        use crate::godot_component::bindgen::godot::core::$m;

        impl $m::Host for GodotCtx {
            fn from(&mut self, val: Vec<$m::Elem>) -> AnyResult<WasmResource<Variant>> {
                Ok(self.set_into_var(&val.into_iter().map(|$v| $e1).collect::<$t>()))
            }

            fn to(&mut self, var: WasmResource<Variant>) -> AnyResult<Vec<$m::Elem>> {
                let v: $t = self.get_var(var).try_to()?;
                Ok(v.as_slice().iter().map(|$v| $e2).collect())
            }

            fn slice(
                &mut self,
                var: WasmResource<Variant>,
                begin: u32,
                end: u32,
            ) -> AnyResult<Vec<$m::Elem>> {
                let v: $t = self.get_var(var).try_to()?;
                let Some(v) = v.as_slice().get(begin as usize..end as usize) else {
                    bail!("index ({begin}..{end}) out of bound")
                };
                Ok(v.iter().map(|$v| $e2).collect())
            }

            fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
                Ok(self.get_var(var).try_to::<$t>()?.len() as _)
            }

            fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
                Ok(self.get_var(var).try_to::<$t>()?.is_empty())
            }

            fn get(&mut self, var: WasmResource<Variant>, i: u32) -> AnyResult<$m::Elem> {
                let v: $t = self.get_var(var).try_to()?;
                let Some($v) = v.as_slice().get(i as usize) else {
                    bail!("index {i} out of bound")
                };
                Ok($e2)
            }

            fn contains(&mut self, var: WasmResource<Variant>, $v: $m::Elem) -> AnyResult<bool> {
                Ok(self.get_var(var).try_to::<$t>()?.contains($e1))
            }

            fn count(&mut self, var: WasmResource<Variant>, $v: $m::Elem) -> AnyResult<u32> {
                Ok(self.get_var(var).try_to::<$t>()?.count($e1) as _)
            }

            fn find(
                &mut self,
                var: WasmResource<Variant>,
                $v: $m::Elem,
                from: Option<u32>,
            ) -> AnyResult<Option<u32>> {
                Ok(self
                    .get_var(var)
                    .try_to::<$t>()?
                    .find($e1, from.map(|v| v as _))
                    .map(|v| v as _))
            }

            fn rfind(
                &mut self,
                var: WasmResource<Variant>,
                $v: $m::Elem,
                from: Option<u32>,
            ) -> AnyResult<Option<u32>> {
                Ok(self
                    .get_var(var)
                    .try_to::<$t>()?
                    .rfind($e1, from.map(|v| v as _))
                    .map(|v| v as _))
            }

            fn subarray(
                &mut self,
                var: WasmResource<Variant>,
                begin: u32,
                end: u32,
            ) -> AnyResult<WasmResource<Variant>> {
                let v: $t = self.get_var(var).try_to()?;
                Ok(self.set_into_var(&v.subarray(begin as _, end as _)))
            }
        }
    };
}

impl_packed_array! {byte_array<PackedByteArray>}
impl_packed_array! {int32_array<PackedInt32Array>}
impl_packed_array! {int64_array<PackedInt64Array>}
impl_packed_array! {float32_array<PackedFloat32Array>}
impl_packed_array! {float64_array<PackedFloat64Array>}
impl_packed_array! {vector2_array<PackedVector2Array> |v| (Vector2 { x: v.x, y: v.y }, vector2_array::Vector2 { x: v.x, y: v.y })}
impl_packed_array! {vector3_array<PackedVector3Array> |v| (Vector3 { x: v.x, y: v.y, z: v.z }, vector3_array::Vector3 { x: v.x, y: v.y, z: v.z })}
impl_packed_array! {color_array<PackedColorArray> |v| (Color { r: v.r, g: v.g, b: v.b, a: v.a }, color_array::Color { r: v.r, g: v.g, b: v.b, a: v.a })}
impl_packed_array! {string_array<PackedStringArray> |v| (GString::from(v), v.to_string())}

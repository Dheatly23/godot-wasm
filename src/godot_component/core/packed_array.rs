use anyhow::{Result as AnyResult, bail};
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;
use crate::godot_component::GodotCtx;

macro_rules! impl_packed_array {
    ($m:ident $s:ident <$t:ty>) => {
        use crate::godot_component::bindgen::godot::core::$m;

        pub mod $s {
            crate::filter_macro!{method [
                from -> "from",
                to -> "to",
                slice -> "slice",
                len -> "len",
                is_empty -> "is-empty",
                get -> "get",
                contains -> "contains",
                count -> "count",
                find -> "find",
                rfind -> "rfind",
                subarray -> "subarray",
            ]}
        }

        impl $m::Host for GodotCtx {
            fn from(&mut self, val: Vec<$m::Elem>) -> AnyResult<WasmResource<Variant>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, from)?;
                self.set_into_var(<$t>::from(&*val))
            }

            fn to(&mut self, var: WasmResource<Variant>) -> AnyResult<Vec<$m::Elem>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, to)?;
                Ok(self.get_value::<$t>(var)?.to_vec())
            }

            fn slice(
                &mut self,
                var: WasmResource<Variant>,
                begin: u32,
                end: u32,
            ) -> AnyResult<Vec<$m::Elem>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, slice)?;
                let v: $t = self.get_value(var)?;
                let Some(v) = v.as_slice().get(begin as usize..end as usize) else {
                    bail!("index ({begin}..{end}) out of bound")
                };
                Ok(v.to_owned())
            }

            fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, len)?;
                Ok(self.get_value::<$t>(var)?.len() as _)
            }

            fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, is_empty)?;
                Ok(self.get_value::<$t>(var)?.is_empty())
            }

            fn get(&mut self, var: WasmResource<Variant>, i: u32) -> AnyResult<$m::Elem> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, get)?;
                let v: $t = self.get_value(var)?;
                let Some(v) = v.as_slice().get(i as usize) else {
                    bail!("index {i} out of bound")
                };
                Ok(*v)
            }

            fn contains(&mut self, var: WasmResource<Variant>, val: $m::Elem) -> AnyResult<bool> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, contains)?;
                Ok(self.get_value::<$t>(var)?.contains(val))
            }

            fn count(&mut self, var: WasmResource<Variant>, val: $m::Elem) -> AnyResult<u32> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, count)?;
                Ok(self.get_value::<$t>(var)?.count(val) as _)
            }

            fn find(
                &mut self,
                var: WasmResource<Variant>,
                val: $m::Elem,
                from: Option<u32>,
            ) -> AnyResult<Option<u32>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, find)?;
                Ok(self.get_value::<$t>(var)?.find(val, from.map(|v| v as _)).map(|v| v as _))
            }

            fn rfind(
                &mut self,
                var: WasmResource<Variant>,
                val: $m::Elem,
                from: Option<u32>,
            ) -> AnyResult<Option<u32>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, rfind)?;
                Ok(self.get_value::<$t>(var)?.rfind(val, from.map(|v| v as _)).map(|v| v as _))
            }

            fn subarray(
                &mut self,
                var: WasmResource<Variant>,
                begin: u32,
                end: u32,
            ) -> AnyResult<WasmResource<Variant>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, subarray)?;
                let v: $t = self.get_value(var)?;
                self.set_into_var(v.subarray(begin as _, end as _))
            }
        }
    };
    ($m:ident $s:ident <$t:ty> |$v:ident|($e1:expr, $e2:expr)) => {
        use crate::godot_component::bindgen::godot::core::$m;

        pub mod $s {
            crate::filter_macro!{method [
                from -> "from",
                to -> "to",
                slice -> "slice",
                len -> "len",
                is_empty -> "is-empty",
                get -> "get",
                contains -> "contains",
                count -> "count",
                find -> "find",
                rfind -> "rfind",
                subarray -> "subarray",
            ]}
        }

        impl $m::Host for GodotCtx {
            fn from(&mut self, val: Vec<$m::Elem>) -> AnyResult<WasmResource<Variant>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, from)?;
                self.set_into_var(val.into_iter().map(|$v| $e1).collect::<$t>())
            }

            fn to(&mut self, var: WasmResource<Variant>) -> AnyResult<Vec<$m::Elem>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, to)?;
                Ok(self.get_value::<$t>(var)?.as_slice().iter().map(|$v| $e2).collect())
            }

            fn slice(
                &mut self,
                var: WasmResource<Variant>,
                begin: u32,
                end: u32,
            ) -> AnyResult<Vec<$m::Elem>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, slice)?;
                let v: $t = self.get_value(var)?;
                let Some(v) = v.as_slice().get(begin as usize..end as usize) else {
                    bail!("index ({begin}..{end}) out of bound")
                };
                Ok(v.iter().map(|$v| $e2).collect())
            }

            fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, len)?;
                Ok(self.get_value::<$t>(var)?.len() as _)
            }

            fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, is_empty)?;
                Ok(self.get_value::<$t>(var)?.is_empty())
            }

            fn get(&mut self, var: WasmResource<Variant>, i: u32) -> AnyResult<$m::Elem> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, get)?;
                let v: $t = self.get_value(var)?;
                let Some($v) = v.as_slice().get(i as usize) else {
                    bail!("index {i} out of bound")
                };
                Ok($e2)
            }

            fn contains(&mut self, var: WasmResource<Variant>, $v: $m::Elem) -> AnyResult<bool> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, contains)?;
                Ok(self.get_value::<$t>(var)?.contains($e1))
            }

            fn count(&mut self, var: WasmResource<Variant>, $v: $m::Elem) -> AnyResult<u32> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, count)?;
                Ok(self.get_value::<$t>(var)?.count($e1) as _)
            }

            fn find(
                &mut self,
                var: WasmResource<Variant>,
                $v: $m::Elem,
                from: Option<u32>,
            ) -> AnyResult<Option<u32>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, find)?;
                Ok(self.get_value::<$t>(var)?.find($e1, from.map(|v| v as _)).map(|v| v as _))
            }

            fn rfind(
                &mut self,
                var: WasmResource<Variant>,
                $v: $m::Elem,
                from: Option<u32>,
            ) -> AnyResult<Option<u32>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, rfind)?;
                Ok(self.get_value::<$t>(var)?.rfind($e1, from.map(|v| v as _)).map(|v| v as _))
            }

            fn subarray(
                &mut self,
                var: WasmResource<Variant>,
                begin: u32,
                end: u32,
            ) -> AnyResult<WasmResource<Variant>> {
                filter_macro!(filter self.filter.as_ref(), godot_core, $m, subarray)?;
                let v: $t = self.get_value(var)?;
                self.set_into_var(v.subarray(begin as _, end as _))
            }
        }
    };
}

impl_packed_array! {byte_array byte_array_filter <PackedByteArray>}
impl_packed_array! {int32_array int32_array_filter <PackedInt32Array>}
impl_packed_array! {int64_array int64_array_filter <PackedInt64Array>}
impl_packed_array! {float32_array float32_array_filter <PackedFloat32Array>}
impl_packed_array! {float64_array float64_array_filter <PackedFloat64Array>}
impl_packed_array! {vector2_array vector2_array_filter <PackedVector2Array> |v| (Vector2 { x: v.x, y: v.y }, vector2_array::Vector2 { x: v.x, y: v.y })}
impl_packed_array! {vector3_array vector3_array_filter <PackedVector3Array> |v| (Vector3 { x: v.x, y: v.y, z: v.z }, vector3_array::Vector3 { x: v.x, y: v.y, z: v.z })}
impl_packed_array! {color_array color_array_filter <PackedColorArray> |v| (Color { r: v.r, g: v.g, b: v.b, a: v.a }, color_array::Color { r: v.r, g: v.g, b: v.b, a: v.a })}

use crate::godot_component::bindgen::godot::core::string_array;

pub mod string_array_filter {
    crate::filter_macro! {method [
        from -> "from",
        to -> "to",
        slice -> "slice",
        len -> "len",
        is_empty -> "is-empty",
        get -> "get",
        contains -> "contains",
        count -> "count",
        find -> "find",
        rfind -> "rfind",
        subarray -> "subarray",
    ]}
}

impl string_array::Host for GodotCtx {
    fn from(&mut self, val: Vec<String>) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, from)?;
        self.set_into_var(
            val.into_iter()
                .map(GString::from)
                .collect::<PackedStringArray>(),
        )
    }

    fn to(&mut self, var: WasmResource<Variant>) -> AnyResult<Vec<String>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, to)?;
        Ok(self
            .get_value::<PackedStringArray>(var)?
            .as_slice()
            .iter()
            .map(|v| v.to_string())
            .collect())
    }

    fn slice(
        &mut self,
        var: WasmResource<Variant>,
        begin: u32,
        end: u32,
    ) -> AnyResult<Vec<String>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, slice)?;
        let v: PackedStringArray = self.get_value(var)?;
        let Some(v) = v.as_slice().get(begin as usize..end as usize) else {
            bail!("index ({begin}..{end}) out of bound")
        };
        Ok(v.iter().map(|v| v.to_string()).collect())
    }

    fn len(&mut self, var: WasmResource<Variant>) -> AnyResult<u32> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, len)?;
        Ok(self.get_value::<PackedStringArray>(var)?.len() as _)
    }

    fn is_empty(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, is_empty)?;
        Ok(self.get_value::<PackedStringArray>(var)?.is_empty())
    }

    fn get(&mut self, var: WasmResource<Variant>, i: u32) -> AnyResult<String> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, get)?;
        let v: PackedStringArray = self.get_value(var)?;
        let Some(v) = v.as_slice().get(i as usize) else {
            bail!("index {i} out of bound")
        };
        Ok(v.to_string())
    }

    fn contains(&mut self, var: WasmResource<Variant>, v: String) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, contains)?;
        Ok(self.get_value::<PackedStringArray>(var)?.contains(&v))
    }

    fn count(&mut self, var: WasmResource<Variant>, v: String) -> AnyResult<u32> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, count)?;
        Ok(self.get_value::<PackedStringArray>(var)?.count(&v) as _)
    }

    fn find(
        &mut self,
        var: WasmResource<Variant>,
        v: String,
        from: Option<u32>,
    ) -> AnyResult<Option<u32>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, find)?;
        Ok(self
            .get_value::<PackedStringArray>(var)?
            .find(&v, from.map(|v| v as _))
            .map(|v| v as _))
    }

    fn rfind(
        &mut self,
        var: WasmResource<Variant>,
        v: String,
        from: Option<u32>,
    ) -> AnyResult<Option<u32>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, rfind)?;
        Ok(self
            .get_value::<PackedStringArray>(var)?
            .rfind(&v, from.map(|v| v as _))
            .map(|v| v as _))
    }

    fn subarray(
        &mut self,
        var: WasmResource<Variant>,
        begin: u32,
        end: u32,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, string_array, subarray)?;
        let v: PackedStringArray = self.get_value(var)?;
        self.set_into_var(v.subarray(begin as _, end as _))
    }
}

use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::filter_macro;
use crate::godot_component::bindgen::godot::core::primitive;

filter_macro! {method [
    from_bool -> "from-bool",
    to_bool -> "to-bool",
    from_int -> "from-int",
    to_int -> "to-int",
    from_float -> "from-float",
    to_float -> "to-float",
    from_vector2 -> "from-vector2",
    to_vector2 -> "to-vector2",
    from_vector3 -> "from-vector3",
    to_vector3 -> "to-vector3",
    from_vector4 -> "from-vector4",
    to_vector4 -> "to-vector4",
    from_vector2i -> "from-vector2i",
    to_vector2i -> "to-vector2i",
    from_vector3i -> "from-vector3i",
    to_vector3i -> "to-vector3i",
    from_vector4i -> "from-vector4i",
    to_vector4i -> "to-vector4i",
    from_rect2 -> "from-rect2",
    to_rect2 -> "to-rect2",
    from_rect2i -> "from-rect2i",
    to_rect2i -> "to-rect2i",
    from_color -> "from-color",
    to_color -> "to-color",
    from_plane -> "from-plane",
    to_plane -> "to-plane",
    from_quaternion -> "from-quaternion",
    to_quaternion -> "to-quaternion",
    from_aabb -> "from-aabb",
    to_aabb -> "to-aabb",
    from_basis -> "from-basis",
    to_basis -> "to-basis",
    from_transform2d -> "from-transform2d",
    to_transform2d -> "to-transform2d",
    from_transform3d -> "from-transform3d",
    to_transform3d -> "to-transform3d",
    from_projection -> "from-projection",
    to_projection -> "to-projection",
    from_string -> "from-string",
    to_string -> "to-string",
    from_stringname -> "from-stringname",
    to_stringname -> "to-stringname",
    from_nodepath -> "from-nodepath",
    to_nodepath -> "to-nodepath",
]}

impl primitive::Host for crate::godot_component::GodotCtx {
    fn from_bool(&mut self, val: bool) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_bool)?;
        self.set_into_var(val)
    }

    fn to_bool(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_bool)?;
        self.get_value(var)
    }

    fn from_int(&mut self, val: i64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_int)?;
        self.set_into_var(val)
    }

    fn to_int(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_int)?;
        self.get_value(var)
    }

    fn from_float(&mut self, val: f64) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_float)?;
        self.set_into_var(val)
    }

    fn to_float(&mut self, var: WasmResource<Variant>) -> AnyResult<f64> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_float)?;
        self.get_value(var)
    }

    fn from_vector2(
        &mut self,
        primitive::Vector2 { x, y }: primitive::Vector2,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_vector2)?;
        self.set_into_var(Vector2 { x, y })
    }

    fn to_vector2(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector2> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_vector2)?;
        let Vector2 { x, y } = self.get_value(var)?;
        Ok(primitive::Vector2 { x, y })
    }

    fn from_vector3(
        &mut self,
        primitive::Vector3 { x, y, z }: primitive::Vector3,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_vector3)?;
        self.set_into_var(Vector3 { x, y, z })
    }

    fn to_vector3(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector3> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_vector3)?;
        let Vector3 { x, y, z } = self.get_value(var)?;
        Ok(primitive::Vector3 { x, y, z })
    }

    fn from_vector4(
        &mut self,
        primitive::Vector4 { x, y, z, w }: primitive::Vector4,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_vector4)?;
        self.set_into_var(Vector4 { x, y, z, w })
    }

    fn to_vector4(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector4> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_vector4)?;
        let Vector4 { x, y, z, w } = self.get_value(var)?;
        Ok(primitive::Vector4 { x, y, z, w })
    }

    fn from_vector2i(
        &mut self,
        primitive::Vector2i { x, y }: primitive::Vector2i,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_vector2i)?;
        self.set_into_var(Vector2i { x, y })
    }

    fn to_vector2i(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector2i> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_vector2i)?;
        let Vector2i { x, y } = self.get_value(var)?;
        Ok(primitive::Vector2i { x, y })
    }

    fn from_vector3i(
        &mut self,
        primitive::Vector3i { x, y, z }: primitive::Vector3i,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_vector3i)?;
        self.set_into_var(Vector3i { x, y, z })
    }

    fn to_vector3i(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector3i> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_vector3i)?;
        let Vector3i { x, y, z } = self.get_value(var)?;
        Ok(primitive::Vector3i { x, y, z })
    }

    fn from_vector4i(
        &mut self,
        primitive::Vector4i { x, y, z, w }: primitive::Vector4i,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_vector4i)?;
        self.set_into_var(Vector4i { x, y, z, w })
    }

    fn to_vector4i(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector4i> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_vector4i)?;
        let Vector4i { x, y, z, w } = self.get_value(var)?;
        Ok(primitive::Vector4i { x, y, z, w })
    }

    fn from_rect2(
        &mut self,
        primitive::Rect2 {
            position: primitive::Vector2 { x: px, y: py },
            size: primitive::Vector2 { x: sx, y: sy },
        }: primitive::Rect2,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_rect2)?;
        let v = Rect2 {
            position: Vector2 { x: px, y: py },
            size: Vector2 { x: sx, y: sy },
        };
        self.set_into_var(v)
    }

    fn to_rect2(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Rect2> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_rect2)?;
        let Rect2 {
            position: Vector2 { x: px, y: py },
            size: Vector2 { x: sx, y: sy },
        } = self.get_value(var)?;
        Ok(primitive::Rect2 {
            position: primitive::Vector2 { x: px, y: py },
            size: primitive::Vector2 { x: sx, y: sy },
        })
    }

    fn from_rect2i(
        &mut self,
        primitive::Rect2i {
            position: primitive::Vector2i { x: px, y: py },
            size: primitive::Vector2i { x: sx, y: sy },
        }: primitive::Rect2i,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_rect2i)?;
        let v = Rect2i {
            position: Vector2i { x: px, y: py },
            size: Vector2i { x: sx, y: sy },
        };
        self.set_into_var(v)
    }

    fn to_rect2i(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Rect2i> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_rect2i)?;
        let Rect2i {
            position: Vector2i { x: px, y: py },
            size: Vector2i { x: sx, y: sy },
        } = self.get_value(var)?;
        Ok(primitive::Rect2i {
            position: primitive::Vector2i { x: px, y: py },
            size: primitive::Vector2i { x: sx, y: sy },
        })
    }

    fn from_color(
        &mut self,
        primitive::Color { r, g, b, a }: primitive::Color,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_color)?;
        self.set_into_var(Color { r, g, b, a })
    }

    fn to_color(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Color> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_color)?;
        let Color { r, g, b, a } = self.get_value(var)?;
        Ok(primitive::Color { r, g, b, a })
    }

    fn from_plane(
        &mut self,
        primitive::Plane {
            normal: primitive::Vector3 { x, y, z },
            d,
        }: primitive::Plane,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_plane)?;
        let v = Plane {
            normal: Vector3 { x, y, z },
            d,
        };
        self.set_into_var(v)
    }

    fn to_plane(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Plane> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_plane)?;
        let Plane {
            normal: Vector3 { x, y, z },
            d,
        } = self.get_value(var)?;
        Ok(primitive::Plane {
            normal: primitive::Vector3 { x, y, z },
            d,
        })
    }

    fn from_quaternion(
        &mut self,
        primitive::Quaternion { x, y, z, w }: primitive::Quaternion,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_quaternion)?;
        self.set_into_var(Quaternion { x, y, z, w })
    }

    fn to_quaternion(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Quaternion> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_quaternion)?;
        let Quaternion { x, y, z, w } = self.get_value(var)?;
        Ok(primitive::Quaternion { x, y, z, w })
    }

    fn from_aabb(
        &mut self,
        primitive::Aabb {
            position:
                primitive::Vector3 {
                    x: px,
                    y: py,
                    z: pz,
                },
            size:
                primitive::Vector3 {
                    x: sx,
                    y: sy,
                    z: sz,
                },
        }: primitive::Aabb,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_aabb)?;
        let v = Aabb {
            position: Vector3 {
                x: px,
                y: py,
                z: pz,
            },
            size: Vector3 {
                x: sx,
                y: sy,
                z: sz,
            },
        };
        self.set_into_var(v)
    }

    fn to_aabb(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Aabb> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_aabb)?;
        let Aabb {
            position:
                Vector3 {
                    x: px,
                    y: py,
                    z: pz,
                },
            size:
                Vector3 {
                    x: sx,
                    y: sy,
                    z: sz,
                },
        } = self.get_value(var)?;
        Ok(primitive::Aabb {
            position: primitive::Vector3 {
                x: px,
                y: py,
                z: pz,
            },
            size: primitive::Vector3 {
                x: sx,
                y: sy,
                z: sz,
            },
        })
    }

    fn from_basis(
        &mut self,
        primitive::Basis {
            col_a:
                primitive::Vector3 {
                    x: ax,
                    y: ay,
                    z: az,
                },
            col_b:
                primitive::Vector3 {
                    x: bx,
                    y: by,
                    z: bz,
                },
            col_c:
                primitive::Vector3 {
                    x: cx,
                    y: cy,
                    z: cz,
                },
        }: primitive::Basis,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_basis)?;
        let v = Basis {
            rows: [
                Vector3 {
                    x: ax,
                    y: bx,
                    z: cx,
                },
                Vector3 {
                    x: ay,
                    y: by,
                    z: cy,
                },
                Vector3 {
                    x: az,
                    y: bz,
                    z: cz,
                },
            ],
        };
        self.set_into_var(v)
    }

    fn to_basis(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Basis> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_basis)?;
        let Basis {
            rows:
                [
                    Vector3 {
                        x: ax,
                        y: bx,
                        z: cx,
                    },
                    Vector3 {
                        x: ay,
                        y: by,
                        z: cy,
                    },
                    Vector3 {
                        x: az,
                        y: bz,
                        z: cz,
                    },
                ],
        } = self.get_value(var)?;
        Ok(primitive::Basis {
            col_a: primitive::Vector3 {
                x: ax,
                y: ay,
                z: az,
            },
            col_b: primitive::Vector3 {
                x: bx,
                y: by,
                z: bz,
            },
            col_c: primitive::Vector3 {
                x: cx,
                y: cy,
                z: cz,
            },
        })
    }

    fn from_transform2d(
        &mut self,
        primitive::Transform2d {
            a: primitive::Vector2 { x: ax, y: ay },
            b: primitive::Vector2 { x: bx, y: by },
            origin: primitive::Vector2 { x: ox, y: oy },
        }: primitive::Transform2d,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_transform2d)?;
        let v = Transform2D {
            a: Vector2 { x: ax, y: ay },
            b: Vector2 { x: bx, y: by },
            origin: Vector2 { x: ox, y: oy },
        };
        self.set_into_var(v)
    }

    fn to_transform2d(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Transform2d> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_transform2d)?;
        let Transform2D {
            a: Vector2 { x: ax, y: ay },
            b: Vector2 { x: bx, y: by },
            origin: Vector2 { x: ox, y: oy },
        } = self.get_value(var)?;
        Ok(primitive::Transform2d {
            a: primitive::Vector2 { x: ax, y: ay },
            b: primitive::Vector2 { x: bx, y: by },
            origin: primitive::Vector2 { x: ox, y: oy },
        })
    }

    fn from_transform3d(
        &mut self,
        primitive::Transform3d {
            basis:
                primitive::Basis {
                    col_a:
                        primitive::Vector3 {
                            x: ax,
                            y: ay,
                            z: az,
                        },
                    col_b:
                        primitive::Vector3 {
                            x: bx,
                            y: by,
                            z: bz,
                        },
                    col_c:
                        primitive::Vector3 {
                            x: cx,
                            y: cy,
                            z: cz,
                        },
                },
            origin:
                primitive::Vector3 {
                    x: ox,
                    y: oy,
                    z: oz,
                },
        }: primitive::Transform3d,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_transform3d)?;
        let v = Transform3D {
            basis: Basis {
                rows: [
                    Vector3 {
                        x: ax,
                        y: bx,
                        z: cx,
                    },
                    Vector3 {
                        x: ay,
                        y: by,
                        z: cy,
                    },
                    Vector3 {
                        x: az,
                        y: bz,
                        z: cz,
                    },
                ],
            },
            origin: Vector3 {
                x: ox,
                y: oy,
                z: oz,
            },
        };
        self.set_into_var(v)
    }

    fn to_transform3d(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Transform3d> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_transform3d)?;
        let Transform3D {
            basis:
                Basis {
                    rows:
                        [
                            Vector3 {
                                x: ax,
                                y: bx,
                                z: cx,
                            },
                            Vector3 {
                                x: ay,
                                y: by,
                                z: cy,
                            },
                            Vector3 {
                                x: az,
                                y: bz,
                                z: cz,
                            },
                        ],
                },
            origin:
                Vector3 {
                    x: ox,
                    y: oy,
                    z: oz,
                },
        } = self.get_value(var)?;
        Ok(primitive::Transform3d {
            basis: primitive::Basis {
                col_a: primitive::Vector3 {
                    x: ax,
                    y: ay,
                    z: az,
                },
                col_b: primitive::Vector3 {
                    x: bx,
                    y: by,
                    z: bz,
                },
                col_c: primitive::Vector3 {
                    x: cx,
                    y: cy,
                    z: cz,
                },
            },
            origin: primitive::Vector3 {
                x: ox,
                y: oy,
                z: oz,
            },
        })
    }

    fn from_projection(
        &mut self,
        primitive::Projection {
            col_a:
                primitive::Vector4 {
                    x: ax,
                    y: ay,
                    z: az,
                    w: aw,
                },
            col_b:
                primitive::Vector4 {
                    x: bx,
                    y: by,
                    z: bz,
                    w: bw,
                },
            col_c:
                primitive::Vector4 {
                    x: cx,
                    y: cy,
                    z: cz,
                    w: cw,
                },
            col_d:
                primitive::Vector4 {
                    x: dx,
                    y: dy,
                    z: dz,
                    w: dw,
                },
        }: primitive::Projection,
    ) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_projection)?;
        let v = Projection {
            cols: [
                Vector4 {
                    x: ax,
                    y: ay,
                    z: az,
                    w: aw,
                },
                Vector4 {
                    x: bx,
                    y: by,
                    z: bz,
                    w: bw,
                },
                Vector4 {
                    x: cx,
                    y: cy,
                    z: cz,
                    w: cw,
                },
                Vector4 {
                    x: dx,
                    y: dy,
                    z: dz,
                    w: dw,
                },
            ],
        };
        self.set_into_var(v)
    }

    fn to_projection(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Projection> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_projection)?;
        let Projection {
            cols:
                [
                    Vector4 {
                        x: ax,
                        y: ay,
                        z: az,
                        w: aw,
                    },
                    Vector4 {
                        x: bx,
                        y: by,
                        z: bz,
                        w: bw,
                    },
                    Vector4 {
                        x: cx,
                        y: cy,
                        z: cz,
                        w: cw,
                    },
                    Vector4 {
                        x: dx,
                        y: dy,
                        z: dz,
                        w: dw,
                    },
                ],
        } = self.get_value(var)?;
        Ok(primitive::Projection {
            col_a: primitive::Vector4 {
                x: ax,
                y: ay,
                z: az,
                w: aw,
            },
            col_b: primitive::Vector4 {
                x: bx,
                y: by,
                z: bz,
                w: bw,
            },
            col_c: primitive::Vector4 {
                x: cx,
                y: cy,
                z: cz,
                w: cw,
            },
            col_d: primitive::Vector4 {
                x: dx,
                y: dy,
                z: dz,
                w: dw,
            },
        })
    }

    fn from_string(&mut self, val: String) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_string)?;
        self.set_into_var(GString::from(val))
    }

    fn to_string(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_string)?;
        Ok(self.get_value::<GString>(var)?.to_string())
    }

    fn from_stringname(&mut self, val: String) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_stringname)?;
        self.set_into_var(StringName::from(val))
    }

    fn to_stringname(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_stringname)?;
        Ok(self.get_value::<StringName>(var)?.to_string())
    }

    fn from_nodepath(&mut self, val: String) -> AnyResult<WasmResource<Variant>> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, from_nodepath)?;
        self.set_into_var(NodePath::from(val))
    }

    fn to_nodepath(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        filter_macro!(filter self.filter.as_ref(), godot_core, primitive, to_nodepath)?;
        Ok(self.get_value::<NodePath>(var)?.to_string())
    }
}

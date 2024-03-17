use anyhow::Result as AnyResult;
use godot::prelude::*;
use wasmtime::component::Resource as WasmResource;

use crate::godot_component::bindgen::godot::core::primitive;
use crate::godot_component::GodotCtx;

impl primitive::Host for GodotCtx {
    fn from_bool(&mut self, val: bool) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&val))
    }

    fn to_bool(&mut self, var: WasmResource<Variant>) -> AnyResult<bool> {
        Ok(self.get_var_borrow(var)?.try_to()?)
    }

    fn from_int(&mut self, val: i64) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&val))
    }

    fn to_int(&mut self, var: WasmResource<Variant>) -> AnyResult<i64> {
        Ok(self.get_var_borrow(var)?.try_to()?)
    }

    fn from_float(&mut self, val: f64) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&val))
    }

    fn to_float(&mut self, var: WasmResource<Variant>) -> AnyResult<f64> {
        Ok(self.get_var_borrow(var)?.try_to()?)
    }

    fn from_vector2(
        &mut self,
        primitive::Vector2 { x, y }: primitive::Vector2,
    ) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&Vector2 { x, y }))
    }

    fn to_vector2(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector2> {
        let Vector2 { x, y } = self.get_var_borrow(var)?.try_to()?;
        Ok(primitive::Vector2 { x, y })
    }

    fn from_vector3(
        &mut self,
        primitive::Vector3 { x, y, z }: primitive::Vector3,
    ) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&Vector3 { x, y, z }))
    }

    fn to_vector3(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector3> {
        let Vector3 { x, y, z } = self.get_var_borrow(var)?.try_to()?;
        Ok(primitive::Vector3 { x, y, z })
    }

    fn from_vector4(
        &mut self,
        primitive::Vector4 { x, y, z, w }: primitive::Vector4,
    ) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&Vector4 { x, y, z, w }))
    }

    fn to_vector4(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector4> {
        let Vector4 { x, y, z, w } = self.get_var_borrow(var)?.try_to()?;
        Ok(primitive::Vector4 { x, y, z, w })
    }

    fn from_vector2i(
        &mut self,
        primitive::Vector2i { x, y }: primitive::Vector2i,
    ) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&Vector2i { x, y }))
    }

    fn to_vector2i(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector2i> {
        let Vector2i { x, y } = self.get_var_borrow(var)?.try_to()?;
        Ok(primitive::Vector2i { x, y })
    }

    fn from_vector3i(
        &mut self,
        primitive::Vector3i { x, y, z }: primitive::Vector3i,
    ) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&Vector3i { x, y, z }))
    }

    fn to_vector3i(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector3i> {
        let Vector3i { x, y, z } = self.get_var_borrow(var)?.try_to()?;
        Ok(primitive::Vector3i { x, y, z })
    }

    fn from_vector4i(
        &mut self,
        primitive::Vector4i { x, y, z, w }: primitive::Vector4i,
    ) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&Vector4i { x, y, z, w }))
    }

    fn to_vector4i(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Vector4i> {
        let Vector4i { x, y, z, w } = self.get_var_borrow(var)?.try_to()?;
        Ok(primitive::Vector4i { x, y, z, w })
    }

    fn from_rect2(
        &mut self,
        primitive::Rect2 {
            position: primitive::Vector2 { x: px, y: py },
            size: primitive::Vector2 { x: sx, y: sy },
        }: primitive::Rect2,
    ) -> AnyResult<WasmResource<Variant>> {
        let v = Rect2 {
            position: Vector2 { x: px, y: py },
            size: Vector2 { x: sx, y: sy },
        };
        Ok(self.set_into_var(&v))
    }

    fn to_rect2(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Rect2> {
        let Rect2 {
            position: Vector2 { x: px, y: py },
            size: Vector2 { x: sx, y: sy },
        } = self.get_var_borrow(var)?.try_to()?;
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
        let v = Rect2i {
            position: Vector2i { x: px, y: py },
            size: Vector2i { x: sx, y: sy },
        };
        Ok(self.set_into_var(&v))
    }

    fn to_rect2i(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Rect2i> {
        let Rect2i {
            position: Vector2i { x: px, y: py },
            size: Vector2i { x: sx, y: sy },
        } = self.get_var_borrow(var)?.try_to()?;
        Ok(primitive::Rect2i {
            position: primitive::Vector2i { x: px, y: py },
            size: primitive::Vector2i { x: sx, y: sy },
        })
    }

    fn from_color(
        &mut self,
        primitive::Color { r, g, b, a }: primitive::Color,
    ) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&Color { r, g, b, a }))
    }

    fn to_color(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Color> {
        let Color { r, g, b, a } = self.get_var_borrow(var)?.try_to()?;
        Ok(primitive::Color { r, g, b, a })
    }

    fn from_plane(
        &mut self,
        primitive::Plane {
            normal: primitive::Vector3 { x, y, z },
            d,
        }: primitive::Plane,
    ) -> AnyResult<WasmResource<Variant>> {
        let v = Plane {
            normal: Vector3 { x, y, z },
            d,
        };
        Ok(self.set_into_var(&v))
    }

    fn to_plane(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Plane> {
        let Plane {
            normal: Vector3 { x, y, z },
            d,
        } = self.get_var_borrow(var)?.try_to()?;
        Ok(primitive::Plane {
            normal: primitive::Vector3 { x, y, z },
            d,
        })
    }

    fn from_quaternion(
        &mut self,
        primitive::Quaternion { x, y, z, w }: primitive::Quaternion,
    ) -> AnyResult<WasmResource<Variant>> {
        Ok(self
            .set_var(Quaternion { x, y, z, w }.to_variant())
            .unwrap())
    }

    fn to_quaternion(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Quaternion> {
        let Quaternion { x, y, z, w } = self.get_var_borrow(var)?.try_to()?;
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
        Ok(self.set_into_var(&v))
    }

    fn to_aabb(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Aabb> {
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
        } = self.get_var_borrow(var)?.try_to()?;
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
        Ok(self.set_into_var(&v))
    }

    fn to_basis(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Basis> {
        let Basis {
            rows:
                [Vector3 {
                    x: ax,
                    y: bx,
                    z: cx,
                }, Vector3 {
                    x: ay,
                    y: by,
                    z: cy,
                }, Vector3 {
                    x: az,
                    y: bz,
                    z: cz,
                }],
        } = self.get_var_borrow(var)?.try_to()?;
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
        let v = Transform2D {
            a: Vector2 { x: ax, y: ay },
            b: Vector2 { x: bx, y: by },
            origin: Vector2 { x: ox, y: oy },
        };
        Ok(self.set_into_var(&v))
    }

    fn to_transform2d(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Transform2d> {
        let Transform2D {
            a: Vector2 { x: ax, y: ay },
            b: Vector2 { x: bx, y: by },
            origin: Vector2 { x: ox, y: oy },
        } = self.get_var_borrow(var)?.try_to()?;
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
        Ok(self.set_into_var(&v))
    }

    fn to_transform3d(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Transform3d> {
        let Transform3D {
            basis:
                Basis {
                    rows:
                        [Vector3 {
                            x: ax,
                            y: bx,
                            z: cx,
                        }, Vector3 {
                            x: ay,
                            y: by,
                            z: cy,
                        }, Vector3 {
                            x: az,
                            y: bz,
                            z: cz,
                        }],
                },
            origin:
                Vector3 {
                    x: ox,
                    y: oy,
                    z: oz,
                },
        } = self.get_var_borrow(var)?.try_to()?;
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
        Ok(self.set_into_var(&v))
    }

    fn to_projection(&mut self, var: WasmResource<Variant>) -> AnyResult<primitive::Projection> {
        let Projection {
            cols:
                [Vector4 {
                    x: ax,
                    y: ay,
                    z: az,
                    w: aw,
                }, Vector4 {
                    x: bx,
                    y: by,
                    z: bz,
                    w: bw,
                }, Vector4 {
                    x: cx,
                    y: cy,
                    z: cz,
                    w: cw,
                }, Vector4 {
                    x: dx,
                    y: dy,
                    z: dz,
                    w: dw,
                }],
        } = self.get_var_borrow(var)?.try_to()?;
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
        Ok(self.set_into_var(&GString::from(val)))
    }

    fn to_string(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        Ok(self.get_var_borrow(var)?.try_to::<GString>()?.to_string())
    }

    fn from_stringname(&mut self, val: String) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&StringName::from(val)))
    }

    fn to_stringname(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        Ok(self
            .get_var_borrow(var)?
            .try_to::<StringName>()?
            .to_string())
    }

    fn from_nodepath(&mut self, val: String) -> AnyResult<WasmResource<Variant>> {
        Ok(self.set_into_var(&NodePath::from(val)))
    }

    fn to_nodepath(&mut self, var: WasmResource<Variant>) -> AnyResult<String> {
        Ok(self.get_var_borrow(var)?.try_to::<NodePath>()?.to_string())
    }
}

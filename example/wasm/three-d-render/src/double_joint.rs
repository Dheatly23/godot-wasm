use std::f32::consts::*;
use std::iter::repeat;

use glam::f32::*;

use crate::{Color, Renderable, State};

#[derive(Debug, Default)]
pub struct DoubleJoint {
    rx: f32,
    ry: f32,
}

const TIME_SCALE: f32 = FRAC_PI_6;
const N_TILES: usize = 4;

impl Renderable for DoubleJoint {
    fn new() -> Self {
        Self {
            rx: 0.0,
            ry: FRAC_PI_2,
        }
    }

    fn step(&mut self, _: f32, delta: f32) {
        self.rx = (self.rx + delta * TIME_SCALE).rem_euclid(TAU);
    }

    fn render(&self, state: &mut State) {
        state.vertex.clear();
        state.normal.clear();
        state.tangent.clear();
        state.uv.clear();
        state.color.clear();
        state.index.clear();

        let (s, c) = self.rx.sin_cos();
        let t = (self.ry * 0.5).tan();

        let b00 = Vec3A::new(-1., -SQRT_2, -1.);
        let b01 = Vec3A::new(-1., -SQRT_2, 1.);
        let b10 = Vec3A::new(1., -SQRT_2, -1.);
        let b11 = Vec3A::new(1., -SQRT_2, 1.);

        let m11 = (s + c) * t;
        let m10 = (s - c) * t;
        let m00 = -m11;
        let m01 = -m10;

        let m00 = Vec3A::new(-1., m00, -1.);
        let m01 = Vec3A::new(-1., m01, 1.);
        let m10 = Vec3A::new(1., m10, -1.);
        let m11 = Vec3A::new(1., m11, 1.);

        let Mat3A {
            x_axis: bx,
            y_axis: mut by,
            z_axis: bz,
        } = Mat3A::from_axis_angle(vec3(-c, 0., s), self.ry);
        by *= SQRT_2;
        let t00 = by - bx - bz;
        let t01 = by - bx + bz;
        let t10 = by + bx - bz;
        let t11 = by + bx + bz;

        const DT: f32 = 1.0 / (N_TILES as f32);

        let mut f = |p00: Vec3A, p01: Vec3A, p10: Vec3A, p11: Vec3A| {
            let n = Vec3::from((p10 - p01).cross(p11 - p00));
            let t = (p01 - p00).extend(1.0);

            for x in 0..N_TILES {
                let x0 = (x as f32) * DT;
                let x1 = x0 + DT;

                let v00 = p00.lerp(p01, x0);
                let v01 = p10.lerp(p11, x0);
                let v10 = p00.lerp(p01, x1);
                let v11 = p10.lerp(p11, x1);

                for y in 0..N_TILES {
                    let y0 = (y as f32) * DT;
                    let y1 = y0 + DT;

                    let i = state.vertex.len();
                    state.vertex.extend([
                        Vec3::from(v00.lerp(v01, y0)),
                        v10.lerp(v11, y0).into(),
                        v00.lerp(v01, y1).into(),
                        v10.lerp(v11, y1).into(),
                    ]);
                    state.normal.extend(repeat(n).take(4));
                    state.tangent.extend(repeat(t).take(4));
                    state
                        .uv
                        .extend([vec2(x0, y0), vec2(x1, y0), vec2(x0, y1), vec2(x1, y1)]);
                    state.color.extend(
                        repeat(if (x + y) % 2 != 0 {
                            Color {
                                r: 1.,
                                g: 1.,
                                b: 1.,
                                a: 1.,
                            }
                        } else {
                            Color {
                                r: 0.,
                                g: 0.,
                                b: 0.,
                                a: 1.,
                            }
                        })
                        .take(4),
                    );
                    state.index.extend([
                        i as u32,
                        (i + 1) as _,
                        (i + 3) as _,
                        i as _,
                        (i + 3) as _,
                        (i + 2) as _,
                    ]);
                }
            }
        };

        f(b01, b00, m01, m00);
        f(m01, m00, t01, t00);
        f(b10, b11, m10, m11);
        f(m10, m11, t10, t11);
        f(b00, b10, m00, m10);
        f(m00, m10, t00, t10);
        f(b11, b01, m11, m01);
        f(m11, m01, t11, t01);
    }
}

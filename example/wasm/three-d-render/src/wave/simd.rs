use std::arch::wasm32::*;
use std::f32::consts::*;
use std::iter::repeat;

use glam::f32::*;

use super::{map_color, MAX_REP, SIZE, SPACE_SCALE, SPEED_SCALE, TIME_SCALE};
use crate::{Renderable, State};

#[derive(Debug, Default)]
pub struct Wave {
    position: Vec<v128>,
    velocity: Vec<v128>,

    width: usize,
    height: usize,

    residue: f32,
}

impl Renderable for Wave {
    fn new() -> Self {
        let mut ret = Self {
            position: vec![f32x4_splat(0.); ((SIZE + 3) >> 2) * SIZE],
            velocity: vec![f32x4_splat(0.); ((SIZE + 3) >> 2) * SIZE],
            width: SIZE,
            height: SIZE,

            residue: 0.0,
        };

        let (ox, oy) = (f32x4_splat(ret.width as f32 * 0.5), ret.height as f32 * 0.5);
        let (dx, dy) = (
            f32x4_splat(SPACE_SCALE / (ret.width - 1) as f32),
            SPACE_SCALE / (ret.height - 1) as f32,
        );
        for (y, a) in ret
            .position
            .chunks_exact_mut((ret.width + 3) >> 2)
            .enumerate()
        {
            for (i, p) in a.iter_mut().enumerate() {
                let mut x = u32x4_add(u32x4_splat((i << 2) as _), u32x4(0, 1, 2, 3));
                let mask = u32x4_lt(x, u32x4_splat(ret.width as _));
                x = f32x4_mul(f32x4_sub(f32x4_convert_u32x4(x), ox), dx);
                let y = (y as f32 - oy) * dy;
                let mut pos = f32x4_sub(f32x4_splat(-(y * y)), f32x4_mul(x, x));
                pos = f32x4(
                    f32x4_extract_lane::<0>(pos).exp(),
                    f32x4_extract_lane::<1>(pos).exp(),
                    f32x4_extract_lane::<2>(pos).exp(),
                    f32x4_extract_lane::<3>(pos).exp(),
                );
                *p = v128_and(f32x4_mul(pos, f32x4_splat(SPACE_SCALE)), mask);
            }
        }

        ret
    }

    fn step(&mut self, _time: f32, mut delta: f32) {
        delta += self.residue;
        let n = (delta.div_euclid(TIME_SCALE) as usize).min(MAX_REP);
        self.residue = delta.rem_euclid(TIME_SCALE);

        let stride = (self.width + 3) >> 2;
        for _ in 0..n {
            let mut prev: &[_] = &[];
            let mut it = self.position.chunks_exact(stride).enumerate().peekable();
            while let Some((mut y, a)) = it.next() {
                y *= stride;
                let next: &[_] = match it.peek() {
                    Some(&(_, a)) => a,
                    None => &[],
                };

                for (i, &cur) in a.iter().enumerate() {
                    let mut d = f32x4_mul(cur, f32x4_splat(-3.0 * SPEED_SCALE));
                    if let Some(&v) = prev.get(i) {
                        d = f32x4_add(d, f32x4_mul(v, f32x4_splat(0.5 * SPEED_SCALE)));
                        d = f32x4_add(
                            d,
                            f32x4_mul(
                                u32x4_shuffle::<3, 4, 5, 6>(
                                    if i > 0 { prev[i - 1] } else { u32x4_splat(0) },
                                    v,
                                ),
                                f32x4_splat(0.25 * SPEED_SCALE),
                            ),
                        );
                        d = f32x4_add(
                            d,
                            f32x4_mul(
                                u32x4_shuffle::<1, 2, 3, 4>(
                                    v,
                                    prev.get(i + 1).copied().unwrap_or(u32x4_splat(0)),
                                ),
                                f32x4_splat(0.25 * SPEED_SCALE),
                            ),
                        );
                    }
                    if let Some(&v) = next.get(i) {
                        d = f32x4_add(d, f32x4_mul(v, f32x4_splat(0.5 * SPEED_SCALE)));
                        d = f32x4_add(
                            d,
                            f32x4_mul(
                                u32x4_shuffle::<3, 4, 5, 6>(
                                    if i > 0 { next[i - 1] } else { u32x4_splat(0) },
                                    v,
                                ),
                                f32x4_splat(0.25 * SPEED_SCALE),
                            ),
                        );
                        d = f32x4_add(
                            d,
                            f32x4_mul(
                                u32x4_shuffle::<1, 2, 3, 4>(
                                    v,
                                    next.get(i + 1).copied().unwrap_or(u32x4_splat(0)),
                                ),
                                f32x4_splat(0.25 * SPEED_SCALE),
                            ),
                        );
                    }

                    d = f32x4_add(
                        d,
                        f32x4_mul(
                            u32x4_shuffle::<3, 4, 5, 6>(
                                if i > 0 { a[i - 1] } else { u32x4_splat(0) },
                                cur,
                            ),
                            f32x4_splat(0.5 * SPEED_SCALE),
                        ),
                    );
                    d = f32x4_add(
                        d,
                        f32x4_mul(
                            u32x4_shuffle::<1, 2, 3, 4>(
                                cur,
                                a.get(i + 1).copied().unwrap_or(u32x4_splat(0)),
                            ),
                            f32x4_splat(0.5 * SPEED_SCALE),
                        ),
                    );
                    if (self.width & 3 != 0) && (i + 1 >= a.len()) {
                        d = v128_and(
                            d,
                            u32x4_lt(u32x4(0, 1, 2, 3), u32x4_splat((self.width & 3) as _)),
                        );
                    }

                    let v = &mut self.velocity[y + i];
                    *v = f32x4_add(*v, d);
                }

                prev = a;
            }

            for (p, v) in self.position.iter_mut().zip(self.velocity.iter()) {
                *p = f32x4_add(*p, f32x4_mul(*v, f32x4_splat(SPEED_SCALE)));
            }
        }
    }

    fn render(&self, state: &mut State) {
        state.vertex.clear();
        state.normal.clear();
        state.tangent.clear();
        state.uv.clear();
        state.color.clear();
        state.index.clear();

        let (dx, dy) = (
            1.0 / (self.width - 1) as f32,
            1.0 / (self.height - 1) as f32,
        );
        let (dx_, dy_) = (dx * SPACE_SCALE, dy * SPACE_SCALE);
        const OFF: f32 = SPACE_SCALE / 2.0;

        let stride = (self.width + 3) >> 2;
        let mut prev: &[_] = &[];
        let mut it = self.position.chunks_exact(stride).enumerate().peekable();
        while let Some((y, a)) = it.next() {
            let mut y = y as f32;
            let v_ = y * dy;
            y = y * dy_ - OFF;
            let next: &[_] = match it.peek() {
                Some(&(_, a)) => a,
                None => &[],
            };

            for (i, &cur) in a.iter().enumerate() {
                let mut v = Mat4::ZERO;
                let f = |a, b| {
                    let mut tx;
                    let ty;

                    tx = f32x4_mul(f32x4_sub(a, b), f32x4_splat(0.5));
                    ty = f32x4_div(
                        f32x4_splat(1.0),
                        f32x4_sqrt(f32x4_add(f32x4_mul(tx, tx), f32x4_splat(1.0))),
                    );
                    tx = f32x4_mul(tx, ty);
                    (Vec4::from(tx), Vec4::from(ty))
                };

                let (tx, ty) = f(
                    cur,
                    if i > 0 {
                        u32x4_shuffle::<3, 4, 5, 6>(a[i - 1], cur)
                    } else {
                        u32x4_shuffle::<0, 0, 1, 2>(cur, cur)
                    },
                );
                v.z_axis += tx;
                v.w_axis += ty;

                let (tx, ty) = f(
                    if i + 1 < a.len() {
                        u32x4_shuffle::<1, 2, 3, 4>(cur, a[i + 1])
                    } else {
                        u32x4_shuffle::<1, 2, 3, 3>(cur, cur)
                    },
                    cur,
                );
                v.z_axis += tx;
                v.w_axis += ty;

                if let Some(&t) = prev.get(i) {
                    let (tx, ty) = f(t, cur);
                    v.x_axis += tx;
                    v.w_axis += ty;
                } else {
                    v.w_axis += Vec4::ONE;
                }

                if let Some(&t) = next.get(i) {
                    let (tx, ty) = f(cur, t);
                    v.x_axis += tx;
                    v.w_axis += ty;
                } else {
                    v.w_axis += Vec4::ONE;
                }

                let t = Vec4::from(f32x4_sqrt(
                    (0..4)
                        .into_iter()
                        .map(|i| v.col(i) * v.col(i))
                        .sum::<Vec4>()
                        .into(),
                ));
                v.x_axis /= t;
                v.y_axis /= t;
                v.z_axis /= t;
                v.w_axis /= t;
                v = v.transpose();

                let mut f = |x, d, q| {
                    let x = x as f32;
                    let q = Quat::from_vec4(q);
                    state.vertex.push(Vec3::new(x * dx_ - OFF, d, y));
                    state.normal.push(q * Vec3::Y);
                    state.tangent.push((q * Vec3A::X).extend(1.0));
                    state.uv.push(Vec2::new(x * dx, v_));
                    state.color.push(map_color(d));
                };

                let x = i << 2;
                f(x, f32x4_extract_lane::<0>(cur), v.x_axis);
                if x + 1 < self.width {
                    f(x + 1, f32x4_extract_lane::<1>(cur), v.y_axis);
                }
                if x + 2 < self.width {
                    f(x + 2, f32x4_extract_lane::<2>(cur), v.z_axis);
                }
                if x + 3 < self.width {
                    f(x + 3, f32x4_extract_lane::<3>(cur), v.w_axis);
                }
            }

            prev = a;
        }

        let mut it = self.position.chunks_exact(stride).enumerate();
        (_, prev) = it.next().unwrap();
        for (y, a) in it {
            let mut y = (y as f32) - 0.5;
            let v_ = y * dy;
            y = y * dy_ - OFF;
            for (i, (&p0, &p2)) in prev.iter().zip(a).enumerate() {
                let x = i << 2;
                if x + 1 == self.width {
                    break;
                }
                let p1 = u32x4_shuffle::<1, 2, 3, 4>(
                    p0,
                    prev.get(i + 1).copied().unwrap_or(u32x4_splat(0)),
                );
                let p3 = u32x4_shuffle::<1, 2, 3, 4>(
                    p2,
                    a.get(i + 1).copied().unwrap_or(u32x4_splat(0)),
                );
                let p4 = f32x4_mul(
                    f32x4_add(f32x4_add(p0, p1), f32x4_add(p2, p3)),
                    f32x4_splat(0.25),
                );

                let mut v = Mat4::ZERO;
                let f = |p: v128| {
                    let mut tx;
                    let ty;

                    tx = f32x4_mul(f32x4_sub(p4, p), f32x4_splat(FRAC_1_SQRT_2));
                    ty = f32x4_div(
                        f32x4_splat(1.0),
                        f32x4_sqrt(f32x4_add(f32x4_mul(tx, tx), f32x4_splat(1.0))),
                    );
                    tx = f32x4_mul(f32x4_mul(tx, ty), f32x4_splat(FRAC_1_SQRT_2));
                    (Vec4::from(tx), Vec4::from(ty))
                };

                let (tx, ty) = f(p0);
                v.x_axis -= tx;
                v.z_axis += tx;
                v.w_axis += ty;

                let (tx, ty) = f(p1);
                v.x_axis -= tx;
                v.z_axis -= tx;
                v.w_axis += ty;

                let (tx, ty) = f(p2);
                v.x_axis += tx;
                v.z_axis += tx;
                v.w_axis += ty;

                let (tx, ty) = f(p3);
                v.x_axis += tx;
                v.z_axis -= tx;
                v.w_axis += ty;

                let t = Vec4::from(f32x4_sqrt(
                    (0..4)
                        .into_iter()
                        .map(|i| v.col(i) * v.col(i))
                        .sum::<Vec4>()
                        .into(),
                ));
                v.x_axis /= t;
                v.y_axis /= t;
                v.z_axis /= t;
                v.w_axis /= t;
                v = v.transpose();

                let mut f = |x, d, q| {
                    let x = (x as f32) + 0.5;
                    let q = Quat::from_vec4(q);
                    state.vertex.push(Vec3::new(x * dx_ - OFF, d, y));
                    state.normal.push(q * Vec3::Y);
                    state.tangent.push((q * Vec3A::X).extend(1.0));
                    state.uv.push(Vec2::new(x * dx, v_));
                    state.color.push(map_color(d));
                };

                f(x, f32x4_extract_lane::<0>(p4), v.x_axis);
                if x + 2 < self.width {
                    f(x + 1, f32x4_extract_lane::<1>(p4), v.y_axis);
                }
                if x + 3 < self.width {
                    f(x + 2, f32x4_extract_lane::<2>(p4), v.z_axis);
                }
                if x + 4 < self.width {
                    f(x + 3, f32x4_extract_lane::<3>(p4), v.w_axis);
                }
            }

            prev = a;
        }

        let e = self.width * self.height;
        for (x, y) in (0..self.height - 1).flat_map(|y| (0..self.width - 1).zip(repeat(y))) {
            let i0 = x * self.width + y;
            let i1 = i0 + 1;
            let i2 = i0 + self.width;
            let i3 = i2 + 1;
            let j = e + x * (self.width - 1) + y;
            state.index.extend([
                i0 as _, i1 as _, j as u32, i1 as _, i3 as _, j as _, i3 as _, i2 as _, j as _,
                i2 as _, i0 as _, j as _,
            ]);
        }
    }
}

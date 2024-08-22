use std::f32::consts::*;
use std::iter::repeat;

use glam::f32::*;
use rand::prelude::*;
use rand_distr::StandardNormal;
use rand_xoshiro::Xoshiro512StarStar;

use super::{map_color, MAX_REP, SIZE, SPACE_SCALE, SPEED_SCALE, TIME_SCALE, WAVE_SCALE};
use crate::{MouseButton, Renderable, State};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct WavePoint {
    position: f32,
    velocity: f32,
}

#[derive(Debug, Default)]
pub struct Wave {
    arr: Vec<WavePoint>,
    width: usize,
    height: usize,

    residue: f32,
    paused: bool,
}

impl Wave {
    fn xy_iter(&self) -> impl Iterator<Item = (usize, usize)> {
        let Self { height, width, .. } = *self;
        (0..height).flat_map(move |y| (0..width).zip(repeat(y)))
    }
}

impl Renderable for Wave {
    fn new() -> Self {
        let mut ret = Self {
            arr: vec![WavePoint::default(); SIZE * SIZE],
            width: SIZE,
            height: SIZE,

            residue: 0.0,
            paused: false,
        };

        let (ox, oy) = (ret.width as f32 * 0.5, ret.height as f32 * 0.5);
        let (dx, dy) = (
            SPACE_SCALE / (ret.width - 1) as f32,
            SPACE_SCALE / (ret.height - 1) as f32,
        );
        for ((x, y), p) in ret.xy_iter().zip(ret.arr.iter_mut()) {
            let x = (x as f32 - ox) * dx;
            let y = (y as f32 - oy) * dy;
            p.position = (-(x * x + y * y)).exp() * SPACE_SCALE;
        }

        ret
    }

    fn step(&mut self, _time: f32, mut delta: f32) {
        delta += self.residue;
        let n = (delta.div_euclid(TIME_SCALE) as usize).min(MAX_REP);
        self.residue = delta.rem_euclid(TIME_SCALE);

        if self.paused {
            return;
        }

        for _ in 0..n {
            for (i, (x, y)) in self.xy_iter().enumerate() {
                let mut d = self.arr[i].position * -3.0;

                let xm = x > 0;
                let xp = x < self.width - 1;
                let ym = y > 0;
                let yp = y < self.height - 1;

                if xm {
                    d += self.arr[i - 1].position * 0.5;
                }
                if xp {
                    d += self.arr[i + 1].position * 0.5;
                }
                if ym {
                    d += self.arr[i - self.width].position * 0.5;
                }
                if yp {
                    d += self.arr[i + self.width].position * 0.5;
                }
                if xm && ym {
                    d += self.arr[i - self.width - 1].position * 0.25;
                }
                if xp && ym {
                    d += self.arr[i - self.width + 1].position * 0.25;
                }
                if xm && yp {
                    d += self.arr[i + self.width - 1].position * 0.25;
                }
                if xp && yp {
                    d += self.arr[i + self.width + 1].position * 0.25;
                }

                self.arr[i].velocity += d * SPEED_SCALE;
            }

            for i in self.arr.iter_mut() {
                i.position += i.velocity * SPEED_SCALE;
            }
        }
    }

    fn click(&mut self, _: Vec3, _: Vec3, button: MouseButton) {
        if let MouseButton::Right = button {
            self.paused = !self.paused;
        } else if let MouseButton::Middle = button {
            let mut rng = Xoshiro512StarStar::from_entropy();

            self.arr.fill(WavePoint::default());

            for i in 0..WAVE_SCALE {
                let ys = (PI / (self.height + 2) as f32) * i as f32;
                for j in 0..WAVE_SCALE {
                    let xs = (PI / (self.width + 2) as f32) * j as f32;

                    let mag = rng.sample::<f32, _>(StandardNormal)
                        * (SPACE_SCALE / WAVE_SCALE as f32).powi(2);
                    for ((x, y), p) in self.xy_iter().zip(self.arr.iter_mut()) {
                        p.position +=
                            (((x + 1) as f32 * xs).cos() + ((y + 1) as f32 * ys).cos()) * mag;
                    }
                }
            }

            for ((x, y), p) in self.xy_iter().zip(self.arr.iter_mut()) {
                let y = (y + 1) as f32 / (self.height + 2) as f32;
                let x = (x + 1) as f32 / (self.width + 2) as f32;
                p.position *= (y - y.powi(2)) * (x - x.powi(2)) * 4.;
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

        for ((x, y), (i, p)) in self.xy_iter().zip(self.arr.iter().enumerate()) {
            let p = p.position;
            let x_ = x as f32;
            let y_ = y as f32;
            state
                .vertex
                .push(Vec3::new(x_ * dx_ - OFF, p, y_ * dy_ - OFF));

            let mut q = Quat::from_array([0.0; 4]);
            q = q + if x > 0 {
                let v = Vec2::new((p - self.arr[i - 1].position) * 0.5, 1.0).normalize();
                Quat::from_xyzw(0.0, 0., v.x, v.y)
            } else {
                Quat::IDENTITY
            };
            q = q + if x < self.width - 1 {
                let v = Vec2::new((self.arr[i + 1].position - p) * 0.5, 1.0).normalize();
                Quat::from_xyzw(0.0, 0., v.x, v.y)
            } else {
                Quat::IDENTITY
            };
            q = q + if y > 0 {
                let v = Vec2::new((self.arr[i - self.width].position - p) * 0.5, 1.0).normalize();
                Quat::from_xyzw(v.x, 0.0, 0.0, v.y)
            } else {
                Quat::IDENTITY
            };
            q = q + if y < self.height - 1 {
                let v = Vec2::new((p - self.arr[i + self.width].position) * 0.5, 1.0).normalize();
                Quat::from_xyzw(v.x, 0.0, 0.0, v.y)
            } else {
                Quat::IDENTITY
            };

            q = q.normalize();
            let n = q * Vec3::Y;
            let t = q * Vec3::X;
            state.normal.push(n);
            state.tangent.push(Vec4::new(t.x, t.y, t.z, 1.0));
            state.uv.push(Vec2::new(x_ * dx, y_ * dy));
            state.color.push(map_color(p));
        }

        for (x, y) in (0..self.height - 1).flat_map(|y| (0..self.width - 1).zip(repeat(y))) {
            let i = x * self.width + y;
            let p0 = self.arr[i].position;
            let p1 = self.arr[i + 1].position;
            let p2 = self.arr[i + self.width].position;
            let p3 = self.arr[i + self.width + 1].position;
            let p4 = (p0 + p1 + p2 + p3) / 4.0;

            let x_ = x as f32 + 0.5;
            let y_ = y as f32 + 0.5;
            state
                .vertex
                .push(Vec3::new(x_ * dx_ - OFF, p4, y_ * dy_ - OFF));

            let mut q = Quat::from_array([0.0; 4]);
            let mut v;
            v = Vec2::new((p4 - p0) * FRAC_1_SQRT_2, 1.0).normalize();
            v.x *= FRAC_1_SQRT_2;
            q = q + Quat::from_xyzw(-v.x, 0.0, v.x, v.y);
            v = Vec2::new((p4 - p1) * FRAC_1_SQRT_2, 1.0).normalize();
            v.x *= FRAC_1_SQRT_2;
            q = q + Quat::from_xyzw(v.x, 0.0, v.x, v.y);
            v = Vec2::new((p4 - p2) * FRAC_1_SQRT_2, 1.0).normalize();
            v.x *= FRAC_1_SQRT_2;
            q = q + Quat::from_xyzw(-v.x, 0.0, -v.x, v.y);
            v = Vec2::new((p4 - p3) * FRAC_1_SQRT_2, 1.0).normalize();
            v.x *= FRAC_1_SQRT_2;
            q = q + Quat::from_xyzw(v.x, 0.0, -v.x, v.y);

            q = q.normalize();
            let n = q * Vec3::Y;
            let t = q * Vec3::X;
            state.normal.push(n);
            state.tangent.push(Vec4::new(t.x, t.y, t.z, 1.0));
            state.uv.push(Vec2::new(x_ * dx, y_ * dy));
            state.color.push(map_color(p4));
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

use std::f32::consts::*;
use std::iter::repeat;

use glam::f32::*;

use super::{Color, Renderable, State};

#[derive(Debug, Default, Clone, Copy, PartialEq)]
struct WavePoint {
    position: f32,
    velocity: f32,
}

#[derive(Debug, Default)]
pub struct Wave {
    arr: Vec<WavePoint>,
    temp: Vec<f32>,
    width: usize,
    height: usize,
}

const SIZE: usize = 16;

impl Renderable for Wave {
    fn new() -> Self {
        let mut ret = Self {
            arr: vec![WavePoint::default(); SIZE * SIZE],
            temp: vec![0.0; SIZE * SIZE],
            width: SIZE,
            height: SIZE,
        };
        let (ox, oy) = (ret.width as f32 * 0.5, ret.height as f32 * 0.5);
        let (dx, dy) = (5.0 / (ret.width - 1) as f32, 5.0 / (ret.height - 1) as f32);
        for (p, (x, y)) in ret.arr.iter_mut().zip(
            (0..ret.height)
                .cycle()
                .flat_map(|y| (0..ret.width).zip(repeat(y))),
        ) {
            let x = (x as f32 - ox) * dx;
            let y = (y as f32 - oy) * dy;
            p.position = (-(x * x + y * y)).exp() * 5.0;
        }

        ret
    }

    fn step(&mut self, _time: f32, delta: f32) {
        for ((i, (p, t)), (x, y)) in self.arr.iter().zip(&mut self.temp).enumerate().zip(
            (0..self.height)
                .cycle()
                .flat_map(|y| (0..self.width).zip(repeat(y))),
        ) {
            let mut d = p.position * -4.0;
            if x > 0 {
                d += self.arr[i - 1].position;
            }
            if x < self.width - 1 {
                d += self.arr[i + 1].position;
            }
            if y > 0 {
                d += self.arr[i - self.width].position;
            }
            if y < self.height - 1 {
                d += self.arr[i + self.width].position;
            }

            *t = d;
        }

        for (i, j) in self.arr.iter_mut().zip(&self.temp) {
            i.position += i.velocity * delta;
            i.velocity += *j * delta;
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
        let (dx_, dy_) = (dx * 5.0, dy * 5.0);

        for ((i, p), (x, y)) in self.arr.iter().enumerate().zip(
            (0..self.height)
                .cycle()
                .flat_map(|y| (0..self.width).zip(repeat(y))),
        ) {
            let p = p.position;
            let x_ = x as f32;
            let y_ = y as f32;
            state.vertex.push(Vec3::new(x_ * dx_, p, y_ * dy_));

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
            state.color.push(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            });
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
            state.vertex.push(Vec3::new(x_ * dx_, p4, y_ * dy_));

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
            state.color.push(Color {
                r: 1.0,
                g: 1.0,
                b: 1.0,
                a: 1.0,
            });
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

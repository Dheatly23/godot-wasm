use std::arch::wasm32::*;
use std::iter::repeat;

use super::{map_color, SIZE, STEPS, XMAX, XMIN, YMAX, YMIN};
use crate::{Color, Renderable, State};

#[derive(Debug, Clone, Copy)]
struct Point {
    cv: v128,
    zv: v128,
    n_iter: usize,
    c: Color,
}

#[derive(Debug, Default)]
pub struct Mandelbrot {
    size: usize,
    points: Vec<Point>,
    steps: usize,
}

impl Renderable for Mandelbrot {
    fn new() -> Self {
        let p = Point {
            cv: f64x2_splat(0.),
            zv: f64x2_splat(0.),
            n_iter: 0,
            c: Color::default(),
        };
        let mut points = vec![p; SIZE * SIZE];

        for (x, p) in points[..SIZE].iter_mut().enumerate() {
            let v = XMIN + ((x as f64) / (SIZE as f64)) * (XMAX - XMIN);
            p.cv = f64x2(v, YMIN);
        }

        for (y, p) in points.chunks_exact_mut(SIZE).enumerate() {
            let v = YMIN + ((y as f64) / (SIZE as f64)) * (YMAX - YMIN);
            let cv = &mut p[0].cv;
            *cv = f64x2_replace_lane::<1>(*cv, v);
        }
        let mut p: &mut [Point] = &mut [];
        for a in points.chunks_exact_mut(SIZE) {
            let v = f64x2_extract_lane::<1>(a[0].cv);
            for (i, j) in a.iter_mut().zip(p) {
                i.cv = f64x2_replace_lane::<1>(j.cv, v);
            }
            p = a;
        }

        for p in &mut points {
            p.zv = p.cv;
        }

        Self {
            size: SIZE,
            points,
            steps: 0,
        }
    }

    fn step(&mut self, _: f32, _: f32) {
        if self.steps >= STEPS {
            return;
        }

        for p in &mut self.points {
            if p.n_iter != self.steps {
                continue;
            }

            const HORIZON: f64 = (1u64 << 40) as f64;

            let v2 = f64x2_mul(p.zv, p.zv);
            let r2 = u64x2_shuffle::<0, 0>(v2, v2);
            let i2 = v128_xor(u64x2_shuffle::<1, 1>(v2, v2), u64x2(1u64 << 63, 0));
            let sd = f64x2_add(r2, i2);
            let v = f64x2_extract_lane::<1>(sd);
            if v >= HORIZON {
                let n = ((p.n_iter + 2) as f64) - v.ln().log2() + HORIZON.ln().log2();
                p.c = map_color(n);
                continue;
            }

            let i = f64x2_extract_lane::<0>(p.zv) * f64x2_extract_lane::<1>(p.zv);
            p.zv = f64x2_add(f64x2_replace_lane::<1>(sd, i + i), p.cv);

            p.n_iter += 1;
        }

        self.steps += 1;
    }

    fn click(&mut self, _: f32, _: f32, _: bool) {}

    fn render(&self, state: &mut State) {
        if self.steps >= STEPS {
            return;
        }

        state.resize(self.size, self.size);
        for (a, c) in self
            .points
            .chunks_exact(self.size)
            .zip(state.colors_mut().chunks_exact_mut(self.size))
        {
            for (p, c) in a.iter().zip(c) {
                if p.n_iter == self.steps {
                    *c = Color {
                        r: 0,
                        g: 0,
                        b: 0,
                        a: 255,
                    };
                    continue;
                }
                *c = p.c;
            }
        }
    }
}

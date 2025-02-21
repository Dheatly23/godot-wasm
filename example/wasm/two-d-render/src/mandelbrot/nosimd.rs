use super::{SIZE, STEPS, XMAX, XMIN, YMAX, YMIN, map_color};
use crate::{Color, MouseButton, Renderable, State};

#[derive(Debug, Default, Clone, Copy)]
struct Point {
    cr: f64,
    ci: f64,
    zr: f64,
    zi: f64,
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
        let mut points = vec![Point::default(); SIZE * SIZE];

        for (x, p) in points[..SIZE].iter_mut().enumerate() {
            let v = (x as f64) / (SIZE as f64);
            p.cr = XMIN + v * (XMAX - XMIN);
        }

        for (y, p) in points.chunks_exact_mut(SIZE).enumerate() {
            let v = (y as f64) / (SIZE as f64);
            p[0].ci = YMIN + v * (YMAX - YMIN);
        }
        let mut p: &mut [Point] = &mut [];
        for a in points.chunks_exact_mut(SIZE) {
            for (i, j) in a.iter_mut().zip(p) {
                i.cr = j.cr;
            }
            let v = a[0].ci;
            for i in &mut a[1..] {
                i.ci = v;
            }
            p = a;
        }

        for p in &mut points {
            (p.zr, p.zi) = (p.cr, p.ci);
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

            let r2 = p.zr.powi(2);
            let i2 = p.zi.powi(2);
            let v = r2 + i2;
            if v >= HORIZON {
                let n = ((p.n_iter + 2) as f64) - v.ln().log2() + HORIZON.ln().log2();
                p.c = map_color(n);
                continue;
            }

            p.zi = (p.zr + p.zr) * p.zi + p.ci;
            p.zr = r2 - i2 + p.cr;

            p.n_iter += 1;
        }

        self.steps += 1;
    }

    fn click(&mut self, _: f32, _: f32, _: MouseButton) {}

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

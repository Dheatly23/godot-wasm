use rand::prelude::*;
use rand_xoshiro::Xoshiro512StarStar;

use super::{MAX_REP, N_PARTICLES, PART_RADIUS, SIZE, SPEED_SCALE_SHR, TIME_SCALE};
use crate::{Color, MouseButton, Renderable, State};

#[derive(Debug, Clone)]
pub struct Particles {
    position: Vec<(i16, i16)>,
    velocity: Vec<(i16, i16)>,

    residue: f32,
    paused: bool,
}

impl Renderable for Particles {
    fn new() -> Self {
        let mut ret = Self {
            position: vec![(0, 0); N_PARTICLES],
            velocity: Vec::new(),

            residue: 0.,
            paused: false,
        };

        ret.velocity = ret.position.clone();
        ret.randomize();
        ret
    }

    fn step(&mut self, _time: f32, mut delta: f32) {
        delta += self.residue;
        let n = (delta.div_euclid(TIME_SCALE) as usize).min(MAX_REP);
        self.residue = delta.rem_euclid(TIME_SCALE);

        if self.paused {
            return;
        }

        #[inline]
        fn process_diff(mut vx: i16, mut vy: i16) -> (i32, i32) {
            if vx == 0 && vy == 0 {
                (vx, vy) = (1, 0);
            }

            // Integer square root.
            // Mostly adapted from https://en.wikipedia.org/wiki/Methods_of_computing_square_roots#Binary_numeral_system_(base_2)
            let mut x = ((vx as i32).pow(2) + (vy as i32).pow(2)) as u32;
            let mut c = 0u32;
            let mut d = 1u32 << ((32 - x.leading_zeros()) & !2).min(30);

            while d != 0 {
                let t = c + d;
                c >>= 1;
                if x >= t {
                    x -= t;
                    c += d;
                }
                d >>= 2;
            }

            let d = (PART_RADIUS - (c as i32 - PART_RADIUS).abs()).max(0);
            ((vx as i32 * d) >> 12, (vy as i32 * d) >> 12)
        }

        for _ in 0..n {
            for ((vx, vy), &(ox, oy)) in self.velocity.iter_mut().zip(&self.position) {
                let mut dx = (*vx as i32) << 8;
                let mut dy = (*vy as i32) << 8;

                for &(px, py) in &self.position {
                    let (x, y) = process_diff(ox - px, oy - py);
                    dx += x;
                    dy += y;
                }

                *vx = (dx >> 8).clamp(-32768, 32767) as _;
                *vy = (dy >> 8).clamp(-32768, 32767) as _;
            }

            for ((px, py), &(vx, vy)) in self.position.iter_mut().zip(&self.velocity) {
                *px = px.wrapping_add(vx >> SPEED_SCALE_SHR);
                *py = py.wrapping_add(vy >> SPEED_SCALE_SHR);
            }
        }
    }

    fn render(&self, state: &mut State) {
        state.resize(SIZE, SIZE);
        let colors = state.colors_mut();
        colors.fill(Color {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        });

        let s = (SIZE as u16).leading_zeros() + 1;
        for &(px, py) in &self.position {
            let x = ((px >> s) as u16 as u32 % SIZE as u32) as usize;
            let y = ((py >> s) as u16 as u32 % SIZE as u32) as usize;
            let c = &mut colors[x + y * SIZE];
            *c = Color {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            };
        }
    }

    fn click(&mut self, _: f32, _: f32, button: MouseButton) {
        if button == MouseButton::Right {
            self.paused = !self.paused;
        } else if button == MouseButton::Middle {
            self.randomize();
        }
    }
}

impl Particles {
    fn randomize(&mut self) {
        self.velocity.fill((0, 0));
        let mut rng = Xoshiro512StarStar::from_os_rng();

        for (px, py) in &mut self.position {
            *px = rng.random();
            *py = rng.random();
        }
    }
}

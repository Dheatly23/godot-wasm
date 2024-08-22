use std::arch::wasm32::*;
use std::slice::from_raw_parts_mut;

use rand::prelude::*;
use rand_xoshiro::Xoshiro512StarStar;

use super::{MAX_REP, N_PARTICLES, PART_RADIUS, SIZE, SPEED_SCALE_SHR, TIME_SCALE};
use crate::{Color, MouseButton, Renderable, State};

#[derive(Debug, Clone)]
pub struct Particles {
    position: Vec<v128>,
    velocity: Vec<v128>,
    len: usize,

    residue: f32,
    paused: bool,
}

impl Renderable for Particles {
    fn new() -> Self {
        let mut ret = Self {
            position: vec![u16x8_splat(0); (N_PARTICLES + 3) >> 2],
            velocity: Vec::new(),
            len: N_PARTICLES,

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
        fn process_diff(v: v128, m: v128) -> (v128, v128) {
            let v = v128_bitselect(
                v,
                u32x4(1, 0xffff0000, 0xffff, 0x10000),
                u32x4_ne(v, u32x4_splat(0)),
            );
            let d = i32x4_dot_i16x8(v, v);
            //let m = v128_and(m, u32x4_lt(d, u32x4_splat(MAX_RADIUS)));

            //crate::print_log(format_args!("v   :{:032x}", print_v128(v)));

            // Integer square root.
            // Mostly adapted from https://en.wikipedia.org/wiki/Methods_of_computing_square_roots#Binary_numeral_system_(base_2)
            let mut x = d;
            let mut c = u32x4_splat(0);
            let mut d = u32x4_splat(1);

            let mut t = x;
            for i in [16, 8, 4, 2] {
                let m = u32x4_ge(t, u32x4_splat(1 << i));
                d = v128_bitselect(u32x4_shl(d, i), d, m);
                t = v128_bitselect(u32x4_shr(t, i), t, m);
            }

            //crate::print_log(format_args!("m   :{:032x}", print_v128(d)));

            for _ in 0..32 {
                if !v128_any_true(d) {
                    break;
                }
                let m = u32x4_ne(d, u32x4_splat(0));

                t = u32x4_add(c, d);
                let m2 = u32x4_ge(x, t);
                x = v128_bitselect(u32x4_sub(x, t), x, m2);
                c = v128_bitselect(u32x4_add(u32x4_shr(c, 1), v128_and(d, m2)), c, m);

                d = u32x4_shr(d, 2);
            }

            //crate::print_log(format_args!("d   :{:032x}", print_v128(c)));

            let d = v128_and(
                i32x4_max(
                    i32x4_sub(
                        i32x4_splat(PART_RADIUS),
                        i32x4_abs(i32x4_sub(c, i32x4_splat(PART_RADIUS))),
                    ),
                    u32x4_splat(0),
                ),
                m,
            );
            let v = i16x8_shuffle::<0, 2, 4, 6, 1, 3, 5, 7>(v, v);
            (
                i32x4_shr(i32x4_mul(i32x4_extend_low_i16x8(v), d), 12),
                i32x4_shr(i32x4_mul(i32x4_extend_high_i16x8(v), d), 12),
            )
        }

        for _ in 0..n {
            //self.velocity.fill(u32x4_splat(0));
            for ((i, v), &o) in self.velocity.iter_mut().enumerate().zip(&self.position) {
                let mut v_ = i16x8_shuffle::<0, 2, 4, 6, 1, 3, 5, 7>(*v, *v);
                let mut dx = i32x4_shl(i32x4_extend_low_i16x8(v_), 8);
                let mut dy = i32x4_shl(i32x4_extend_high_i16x8(v_), 8);

                //crate::print_log(format_args!("dx  :{:032x}", print_v128(dx)));
                //crate::print_log(format_args!("dy  :{:032x}", print_v128(dy)));

                for (j, &p) in self.position.iter().enumerate() {
                    let m = if i == j { 0 } else { -1 };
                    if j * 4 < self.len {
                        let (x, y) = process_diff(
                            i16x8_sub(u32x4_splat(u32x4_extract_lane::<0>(p)), o),
                            i32x4(m, -1, -1, -1),
                        );
                        dx = i32x4_add(dx, x);
                        dy = i32x4_add(dy, y);
                    }
                    if j * 4 + 1 < self.len {
                        let (x, y) = process_diff(
                            i16x8_sub(u32x4_splat(u32x4_extract_lane::<1>(p)), o),
                            i32x4(-1, m, -1, -1),
                        );
                        dx = i32x4_add(dx, x);
                        dy = i32x4_add(dy, y);
                    }
                    if j * 4 + 2 < self.len {
                        let (x, y) = process_diff(
                            i16x8_sub(u32x4_splat(u32x4_extract_lane::<2>(p)), o),
                            i32x4(-1, -1, m, -1),
                        );
                        dx = i32x4_add(dx, x);
                        dy = i32x4_add(dy, y);
                    }
                    if j * 4 + 3 < self.len {
                        let (x, y) = process_diff(
                            i16x8_sub(u32x4_splat(u32x4_extract_lane::<3>(p)), o),
                            i32x4(-1, -1, -1, m),
                        );
                        dx = i32x4_add(dx, x);
                        dy = i32x4_add(dy, y);
                    }
                }

                //crate::print_log(format_args!("dx  :{:032x}", print_v128(dx)));
                //crate::print_log(format_args!("dy  :{:032x}", print_v128(dy)));

                v_ = i16x8_narrow_i32x4(i32x4_shr(dx, 8), i32x4_shr(dy, 8));
                *v = i16x8_shuffle::<0, 4, 1, 5, 2, 6, 3, 7>(v_, v_);
            }

            //crate::print_log(format_args!("v[0]:{:032x}", print_v128(self.velocity[0])));
            if self.len & 3 != 0 {
                let v = self.velocity.last_mut().unwrap();
                *v = v128_and(
                    *v,
                    u32x4_lt(u32x4(0, 1, 2, 3), u32x4_splat((self.len & 3) as _)),
                );
            }

            for (p, v) in self.position.iter_mut().zip(&self.velocity) {
                *p = i16x8_add(*p, i16x8_shr(*v, SPEED_SCALE_SHR));
            }

            //crate::print_log(format_args!("p[0]:{:032x}", print_v128(self.position[0])));
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

        for (i, p) in self.position.iter().enumerate() {
            let p = u16x8_shr(*p, (SIZE as u16).leading_zeros() + 1);
            let a = [
                u32x4_extract_lane::<0>(p),
                u32x4_extract_lane::<1>(p),
                u32x4_extract_lane::<2>(p),
                u32x4_extract_lane::<3>(p),
            ];
            let s = if i + 4 > self.len {
                &a[..self.len & 3]
            } else {
                &a[..]
            };
            for &i in s {
                let x = (i % SIZE as u32) as usize;
                let y = ((i >> 16) % SIZE as u32) as usize;
                let c = &mut colors[x + y * SIZE];
                *c = Color {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                };
            }
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
        self.velocity.fill(u32x4_splat(0));
        let mut rng = Xoshiro512StarStar::from_entropy();
        // SAFETY: Layout matches.
        unsafe {
            rng.fill_bytes(from_raw_parts_mut(
                self.position.as_mut_ptr() as *mut u8,
                self.len * 4,
            ));
        }

        if self.len & 3 != 0 {
            let p = self.position.last_mut().unwrap();
            *p = v128_and(
                *p,
                u32x4_lt(u32x4(0, 1, 2, 3), u32x4_splat((self.len & 3) as _)),
            );
        }
    }
}

#[allow(dead_code)]
fn print_v128(v: v128) -> u128 {
    u64x2_extract_lane::<0>(v) as u128 | (u64x2_extract_lane::<1>(v) as u128) << 64
}

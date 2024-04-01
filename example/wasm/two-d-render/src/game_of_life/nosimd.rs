use std::iter::repeat;
use std::mem::size_of_val;
use std::slice::from_raw_parts_mut;

use rand_xoshiro::rand_core::{RngCore, SeedableRng};
use rand_xoshiro::Xoshiro512StarStar;

use super::SIZE;
use crate::{log, Color, MouseButton, Renderable, State};

#[derive(Debug, Default)]
pub struct GameOfLife {
    size: usize,
    data: Vec<u16>,
    paused: bool,
}

fn to_vec(v: u16) -> u64 {
    let v = v as u64;
    (v | v << 15 | v << 30 | v << 45) & 0x1111_1111_1111_1111
}

fn from_vec(mut v: u64) -> u16 {
    v &= 0x1111_1111_1111_1111;
    (v | v >> 15 | v >> 30 | v >> 45) as u16
}

fn apply_rule(v: u64, r: u64) -> u64 {
    let mut ret = v;

    // Either 2 or 3
    let mut x = r ^ 0xcccc_cccc_cccc_cccc;
    x = x >> 2 & x >> 1;
    x = x & x >> 1 & 0x1111_1111_1111_1111;

    // Death
    ret &= x;
    // Reproduction
    ret |= x & r;

    ret
}

const fn rot_y_neg(v: u64) -> u64 {
    v >> 4 & 0x0fff_0fff_0fff_0fff | v << 12 & 0xf000_f000_f000_f000
}

const fn rot_y_pos(v: u64) -> u64 {
    v << 4 & 0xfff0_fff0_fff0_fff0 | v >> 12 & 0x000f_000f_000f_000f
}

impl Renderable for GameOfLife {
    fn new() -> Self {
        let size = (SIZE + 7) >> 3 << 1;
        let data = vec![0; size * size * 2];

        Self {
            size,
            data,
            paused: true,
        }
    }

    fn step(&mut self, _: f32, _: f32) {
        if self.paused {
            return;
        }

        let Self {
            data: ref mut d,
            size: s,
            ..
        } = *self;
        let l = d.len() / 2;
        debug_assert_eq!(d.len(), s * s * 2);
        debug_assert_eq!(s & 1, 0);
        log!("s: {s}");

        let mut it = 0..s / 2;
        while let Some(i) = it.next() {
            let endy = it.is_empty();
            let i = i * 2 * s;

            let mut it = 0..s / 2;
            while let Some(j) = it.next() {
                let endx = it.is_empty();
                let j = j * 2;
                let ix = i + j;
                log!("i: {i} j: {j}");

                let v00 = to_vec(d[ix]);
                let v01 = to_vec(d[ix + 1]);
                let v10 = to_vec(d[ix + s]);
                let v11 = to_vec(d[ix + s + 1]);
                let mut r00 = v01 + v11 + v10;
                let mut r01 = v00 + v10 + v11;
                let mut r10 = v00 + v01 + v11;
                let mut r11 = v01 + v00 + v10;
                log!("v00: {v00:016X} v01: {v01:016X} v10: {v10:016X} v11: {v11:016X}");

                let mut ix_;
                let mut o;

                ix_ = if j == 0 { ix + s - 1 } else { ix - 1 };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]) + to_vec(d[ix_ + s]);
                if j == 0 {
                    o = o.rotate_left(16);
                }
                r00 += o;
                r10 += o;
                log!("o: {o:016X}");

                ix_ = if endx { i } else { ix + 2 };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]) + to_vec(d[ix_ + s]);
                if endx {
                    o = o.rotate_right(16);
                }
                r01 += o;
                r11 += o;
                log!("o: {o:016X}");

                ix_ = if i == 0 { l - s + j } else { ix - s };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]) + to_vec(d[ix_ + 1]);
                if i == 0 {
                    o = rot_y_pos(o);
                }
                r00 += o;
                r01 += o;
                log!("o: {o:016X}");

                ix_ = if endy { j } else { ix + s * 2 };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]) + to_vec(d[ix_ + 1]);
                if endy {
                    o = rot_y_neg(o);
                }
                r10 += o;
                r11 += o;
                log!("o: {o:016X}");

                ix_ = match (i == 0, j == 0) {
                    (false, false) => ix - s - 1,
                    (false, true) => ix - 1,
                    (true, false) => l - 1 - s + j,
                    (true, true) => l - 1,
                };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]);
                if i == 0 {
                    o = rot_y_pos(o);
                }
                if j == 0 {
                    o = o.rotate_left(16);
                }
                r00 += o;
                log!("o: {o:016X}");

                ix_ = match (endy, j == 0) {
                    (false, false) => ix + s * 2 - 1,
                    (false, true) => ix + s * 3 - 1,
                    (true, false) => j - 1,
                    (true, true) => s - 1,
                };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]);
                if endy {
                    o = rot_y_neg(o);
                }
                if j == 0 {
                    o = o.rotate_left(16);
                }
                r10 += o;
                log!("o: {o:016X}");

                ix_ = match (i == 0, endx) {
                    (false, false) => ix + 2 - s,
                    (false, true) => ix + 2 - s * 2,
                    (true, false) => l - s + j + 2,
                    (true, true) => l - s,
                };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]);
                if i == 0 {
                    o = rot_y_pos(o);
                }
                if endx {
                    o = o.rotate_right(16);
                }
                r01 += o;
                log!("o: {o:016X}");

                ix_ = match (endy, endx) {
                    (false, false) => ix + 2 + s * 2,
                    (false, true) => ix + 2 + s,
                    (true, false) => j + 2,
                    (true, true) => 0,
                };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]);
                if endy {
                    o = rot_y_neg(o);
                }
                if endx {
                    o = o.rotate_right(16);
                }
                r11 += o;
                log!("o: {o:016X}");

                let o00 = apply_rule(v00, r00);
                let o01 = apply_rule(v01, r01);
                let o10 = apply_rule(v10, r10);
                let o11 = apply_rule(v11, r11);
                log!("v: {v00:016X} r: {r00:016X} o: {o00:016X}");
                log!("v: {v01:016X} r: {r01:016X} o: {o01:016X}");
                log!("v: {v10:016X} r: {r10:016X} o: {o10:016X}");
                log!("v: {v11:016X} r: {r11:016X} o: {o11:016X}");
                d[ix + l] = from_vec(o00);
                d[ix + l + 1] = from_vec(o01);
                d[ix + l + s] = from_vec(o10);
                d[ix + l + s + 1] = from_vec(o11);
                log!(
                    "output: {:04X} {:04X} {:04X} {:04X}",
                    d[ix + l],
                    d[ix + l + 1],
                    d[ix + l + s],
                    d[ix + l + s + 1]
                );
            }
        }

        d.copy_within(l.., 0);
    }

    fn click(&mut self, x: f32, y: f32, button: MouseButton) {
        if let MouseButton::Right = button {
            self.paused = !self.paused;
        } else if let MouseButton::Middle = button {
            let mut rng = Xoshiro512StarStar::from_entropy();
            // SAFETY: All bounds are valid.
            unsafe {
                rng.fill_bytes(from_raw_parts_mut(
                    self.data.as_mut_ptr() as *mut u8,
                    size_of_val(&self.data[..]),
                ))
            }
        } else if let MouseButton::Left = button {
            let (x, y) = ((x / 4.) as i32, (y / 4.) as i32);
            let r = 0..(self.size << 2) as i32;
            if !r.contains(&x) || !r.contains(&y) {
                return;
            }

            let (x, y) = (x as usize, y as usize);
            let i = x % self.size + (y % self.size) * self.size;
            let b = x / self.size | (y / self.size) << 2;
            self.data[i] ^= 1 << b;
        }
    }

    fn render(&self, state: &mut State) {
        state.resize(self.size << 4, self.size << 4);

        static PATTERN: [u8; 16] = [8, 9, 10, 11, 0, 1, 2, 3, 0, 1, 2, 3, 4, 5, 6, 7];

        let mut it = state.colors_mut().into_iter();
        for sy in 0..4 {
            let mut b = true;
            for a in self.data[..self.data.len() / 2].chunks_exact(self.size) {
                let b_ = b;
                b = false;
                for b in PATTERN {
                    let sx = (b & 3) as u32;
                    let b = b & 4 != 0 || b & 8 != 0 && b_;
                    for (i, &v) in a.iter().enumerate() {
                        let v = v & 1 << (sx + sy * 4) != 0;
                        for b in [b || i == 0, b, b, true] {
                            let c = it.next().unwrap();
                            let v = match (v, b) {
                                (false, false) => 0,
                                (false, true) => 64,
                                (true, false) => 255,
                                (true, true) => 191,
                            };
                            *c = Color {
                                r: v,
                                g: v,
                                b: v,
                                a: 255,
                            };
                        }
                    }
                }
            }
        }
    }
}

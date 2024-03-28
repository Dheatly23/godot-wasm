use std::arch::wasm32::*;
use std::iter::repeat;

use super::SIZE;
use crate::{log, Color, Renderable, State};

#[derive(Debug, Default)]
pub struct GameOfLife {
    size: usize,
    data: Vec<u32>,
    paused: bool,
}

fn to_vec(v: u32) -> v128 {
    let mut v1 = u32x4_mul(u32x4_splat(v), u32x4(1, 2, 4, 8));
    let v2 = u8x16_shr(u8x16_shl(v1, 4), 7);
    v1 = u8x16_shr(v1, 7);

    let ol = u8x16_shuffle::<28, 20, 12, 4, 29, 21, 13, 5, 30, 22, 14, 6, 31, 23, 15, 7>(v1, v2);
    let oh = u8x16_shuffle::<24, 16, 8, 0, 25, 17, 9, 1, 26, 18, 10, 2, 27, 19, 11, 3>(v1, v2);
    v128_or(u8x16_shl(oh, 4), ol)
}

fn from_vec(v: v128) -> u32 {
    let v1 = u8x16_shl(v, 7);
    let v2 = u8x16_shl(v, 3);

    let ol = u8x16_shuffle::<0, 16, 1, 17, 2, 18, 3, 19, 4, 20, 5, 21, 6, 22, 7, 23>(v1, v2);
    let oh = u8x16_shuffle::<8, 24, 9, 25, 10, 26, 11, 27, 12, 28, 13, 29, 14, 30, 15, 31>(v1, v2);
    (u8x16_bitmask(ol) as u32) | ((u8x16_bitmask(oh) as u32) << 16)
}

fn apply_rule(v: v128, r: v128) -> v128 {
    // Either 2 or 3
    let xl = u8x16_eq(v128_and(r, u8x16_splat(0x0e)), u8x16_splat(0x02));
    let xh = u8x16_eq(v128_and(r, u8x16_splat(0xe0)), u8x16_splat(0x20));
    let x = v128_and(v128_bitselect(xl, xh, u8x16_splat(0x0f)), u8x16_splat(0x11));

    // Death
    let v = v128_and(v, x);
    // Reproduction
    let v = v128_or(v, v128_and(x, r));

    v
}

#[inline]
fn rot_x_neg(v: v128) -> v128 {
    v128_or(u32x4_shr(v, 4), u32x4_shl(v, 28))
}

#[inline]
fn rot_x_pos(v: v128) -> v128 {
    v128_or(u32x4_shl(v, 4), u32x4_shr(v, 28))
}

#[inline]
fn rot_y_neg(v: v128) -> v128 {
    u32x4_shuffle::<1, 2, 3, 0>(v, v)
}

#[inline]
fn rot_y_pos(v: v128) -> v128 {
    u32x4_shuffle::<3, 0, 1, 2>(v, v)
}

impl Renderable for GameOfLife {
    fn new() -> Self {
        let size = (SIZE + 31) >> 5 << 2;
        let data = vec![0; size * size * 4];

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

        let d = &mut self.data;
        let l = d.len() / 2;
        let sx = self.size;
        let sy = self.size * 2;
        debug_assert_eq!(d.len(), sx * sy * 2);
        debug_assert_eq!(self.size & 3, 0);

        log!("sx: {sx} sy: {sy}");

        let mut it = 0..sy / 2;
        while let Some(i) = it.next() {
            let endy = it.is_empty();
            let i = i * 2 * sx;

            let mut it = 0..sx / 2;
            while let Some(j) = it.next() {
                let endx = it.is_empty();
                let j = j * 2;
                let ix = i + j;
                log!("i: {i} j: {j}");

                let v00 = to_vec(d[ix]);
                let v01 = to_vec(d[ix + 1]);
                let v10 = to_vec(d[ix + sx]);
                let v11 = to_vec(d[ix + sx + 1]);
                let mut r00 = u8x16_add(u8x16_add(v01, v11), v10);
                let mut r01 = u8x16_add(u8x16_add(v00, v10), v11);
                let mut r10 = u8x16_add(u8x16_add(v00, v01), v11);
                let mut r11 = u8x16_add(u8x16_add(v01, v00), v10);
                log!(
                    "v00: {:032X} v01: {:032X} v10: {:032X} v11: {:032X}",
                    print_v128(v00),
                    print_v128(v01),
                    print_v128(v10),
                    print_v128(v11)
                );

                let mut ix_;
                let mut o;

                ix_ = if j == 0 { ix + sx - 1 } else { ix - 1 };
                log!("ix: {ix_}");
                o = u8x16_add(to_vec(d[ix_]), to_vec(d[ix_ + sx]));
                if j == 0 {
                    o = rot_x_pos(o);
                }
                r00 = u8x16_add(r00, o);
                r10 = u8x16_add(r10, o);
                log!("o: {:032X}", print_v128(o));

                ix_ = if endx { i } else { ix + 2 };
                log!("ix: {ix_}");
                o = u8x16_add(to_vec(d[ix_]), to_vec(d[ix_ + sx]));
                if endx {
                    o = rot_x_neg(o);
                }
                r01 = u8x16_add(r01, o);
                r11 = u8x16_add(r11, o);
                log!("o: {:032X}", print_v128(o));

                ix_ = if i == 0 { l - sx + j } else { ix - sx };
                log!("ix: {ix_}");
                o = u8x16_add(to_vec(d[ix_]), to_vec(d[ix_ + 1]));
                if i == 0 {
                    o = rot_y_pos(o);
                }
                r00 = u8x16_add(r00, o);
                r01 = u8x16_add(r01, o);
                log!("o: {:032X}", print_v128(o));

                ix_ = if endy { j } else { ix + sx * 2 };
                log!("ix: {ix_}");
                o = u8x16_add(to_vec(d[ix_]), to_vec(d[ix_ + 1]));
                if endy {
                    o = rot_y_neg(o);
                }
                r10 = u8x16_add(r10, o);
                r11 = u8x16_add(r11, o);
                log!("o: {:032X}", print_v128(o));

                ix_ = match (i == 0, j == 0) {
                    (false, false) => ix - sx - 1,
                    (false, true) => ix - 1,
                    (true, false) => l - 1 - sx + j,
                    (true, true) => l - 1,
                };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]);
                if i == 0 {
                    o = rot_y_pos(o);
                }
                if j == 0 {
                    o = rot_x_pos(o);
                }
                r00 = u8x16_add(r00, o);
                log!("o: {:032X}", print_v128(o));

                ix_ = match (endy, j == 0) {
                    (false, false) => ix + sx * 2 - 1,
                    (false, true) => ix + sx * 3 - 1,
                    (true, false) => j - 1,
                    (true, true) => sx - 1,
                };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]);
                if endy {
                    o = rot_y_neg(o);
                }
                if j == 0 {
                    o = rot_x_pos(o);
                }
                r10 = u8x16_add(r10, o);
                log!("o: {:032X}", print_v128(o));

                ix_ = match (i == 0, endx) {
                    (false, false) => ix + 2 - sx,
                    (false, true) => ix + 2 - sx * 2,
                    (true, false) => l - sx + j + 2,
                    (true, true) => l - sx,
                };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]);
                if i == 0 {
                    o = rot_y_pos(o);
                }
                if endx {
                    o = rot_x_neg(o);
                }
                r01 = u8x16_add(r01, o);
                log!("o: {:032X}", print_v128(o));

                ix_ = match (endy, endx) {
                    (false, false) => ix + 2 + sx * 2,
                    (false, true) => ix + 2 + sx,
                    (true, false) => j + 2,
                    (true, true) => 0,
                };
                log!("ix: {ix_}");
                o = to_vec(d[ix_]);
                if endy {
                    o = rot_y_neg(o);
                }
                if endx {
                    o = rot_x_neg(o);
                }
                r11 = u8x16_add(r11, o);
                log!("o: {:032X}", print_v128(o));

                let o00 = apply_rule(v00, r00);
                let o01 = apply_rule(v01, r01);
                let o10 = apply_rule(v10, r10);
                let o11 = apply_rule(v11, r11);
                log!(
                    "v: {:032X} r: {:032X} o: {:032X}",
                    print_v128(v00),
                    print_v128(r00),
                    print_v128(o00)
                );
                log!(
                    "v: {:032X} r: {:032X} o: {:032X}",
                    print_v128(v01),
                    print_v128(r01),
                    print_v128(o01)
                );
                log!(
                    "v: {:032X} r: {:032X} o: {:032X}",
                    print_v128(v10),
                    print_v128(r10),
                    print_v128(o10)
                );
                log!(
                    "v: {:032X} r: {:032X} o: {:032X}",
                    print_v128(v11),
                    print_v128(r11),
                    print_v128(o11)
                );
                d[ix + l] = from_vec(o00);
                d[ix + l + 1] = from_vec(o01);
                d[ix + l + sx] = from_vec(o10);
                d[ix + l + sx + 1] = from_vec(o11);
                log!(
                    "output: {:08X} {:08X} {:08X} {:08X}",
                    d[ix + l],
                    d[ix + l + 1],
                    d[ix + l + sx],
                    d[ix + l + sx + 1]
                );
            }
        }

        d.copy_within(l.., 0);
    }

    fn click(&mut self, x: f32, y: f32, right_click: bool) {
        if right_click {
            self.paused = !self.paused;
            return;
        }

        let (x, y) = ((x / 4.) as i32, (y / 4.) as i32);
        let r = 0..(self.size << 3) as i32;
        if !r.contains(&x) || !r.contains(&y) {
            return;
        }

        let (x, y) = (x as usize, y as usize);
        let i = x % self.size + (y % (self.size * 2)) * self.size;
        let b = x / self.size | (y / (self.size * 2)) << 3;
        self.data[i] ^= 1 << b;
    }

    fn render(&self, state: &mut State) {
        state.resize(self.size << 5, self.size << 5);

        const COLORMAP: v128 = u8x16(0, 64, 255, 191, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0);
        static PATTERN: [u8; 32] = [
            16, 17, 18, 19, 20, 21, 22, 23, 0, 1, 2, 3, 4, 5, 6, 7, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9,
            10, 11, 12, 13, 14, 15,
        ];

        let mut it = state.colors_mut().into_iter();
        for sy in 0..4 {
            let mut b = true;
            for a in self.data[..self.data.len() / 2].chunks_exact(self.size) {
                let b_ = b;
                b = false;
                for b in PATTERN {
                    let sx = (b & 7) as u32;
                    let b = b & 8 != 0 || b & 16 != 0 && b_;
                    for (i, &v) in a.iter().enumerate() {
                        let v = v & 1 << (sx + sy * 8) != 0;

                        let c = it.next().unwrap() as *mut _;
                        for _ in 0..3 {
                            it.next().unwrap();
                        }
                        let mut v = v128_or(
                            u8x16_splat(if v { 2 } else { 0 }),
                            if b {
                                u8x16_splat(1)
                            } else if i == 0 {
                                u32x4(0x0101_0101, 0, 0, 0x0101_0101)
                            } else {
                                u32x4(0, 0, 0, 0x0101_0101)
                            },
                        );
                        v = v128_or(u8x16_swizzle(COLORMAP, v), u32x4_splat(0xff00_0000));
                        // SAFETY: Color struct is 4 byte, and there are 4 adjacent.
                        unsafe { v128_store(c as _, v) }
                    }
                }
            }
        }
    }
}

fn print_v128(v: v128) -> u128 {
    u64x2_extract_lane::<0>(v) as u128 | (u64x2_extract_lane::<1>(v) as u128) << 64
}

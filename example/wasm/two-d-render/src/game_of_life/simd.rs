use std::arch::wasm32::*;
use std::iter::repeat;

use super::SIZE;
use crate::{log, Color, Renderable, State};

#[derive(Debug, Default)]
pub struct GameOfLife {
    size: usize,
    data: Vec<(u32, u32)>,
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

fn apply_rule(mut v: v128, r: v128) -> v128 {
    let rl = v128_and(r, u8x16_splat(0x0f));
    let rh = u8x16_shr(r, 4);

    let mut bl = v128_and(u8x16_gt(rl, u8x16_splat(1)), u8x16_le(rl, u8x16_splat(3)));
    let mut bh = v128_and(u8x16_gt(rh, u8x16_splat(1)), u8x16_le(rh, u8x16_splat(3)));
    v = v128_and(v, v128_bitselect(bl, bh, u8x16_splat(0x0f)));

    bl = u8x16_eq(rl, u8x16_splat(3));
    bh = u8x16_eq(rh, u8x16_splat(3));
    v128_or(
        v,
        v128_and(v128_bitselect(bl, bh, u8x16_splat(0x0f)), u8x16_splat(0x11)),
    )
}

fn _u8x16_add(a: v128, b: v128) -> v128 {
    u8x16_add(a, b)
}

impl Renderable for GameOfLife {
    fn new() -> Self {
        let data = vec![(0, 0); ((SIZE + 7) >> 3) * ((SIZE + 3) >> 2)];

        Self {
            size: SIZE,
            data,
            paused: true,
        }
    }

    fn step(&mut self, _: f32, _: f32) {
        if self.paused {
            return;
        }

        let sx = (self.size + 7) >> 3;
        let sy = (self.size + 3) >> 2;
        let endrow = sx * (sy - 1);
        debug_assert_eq!(self.data.len(), sx * sy);

        log!("sx: {sx} sy: {sy}");

        let lx = self.size & 7;
        let ly = self.size & 3;
        log!("lx: {lx} ly: {ly}");

        if lx != 0 {
            let mut m = (255u8 << lx) as u32;
            m = m | m << 8 | m << 16 | m << 24;
            let mi = !m;
            for mut i in 0..sy {
                i *= sx;
                let j = sx - 1 + i;
                let v = self.data[j].0;
                let o = self.data[i].0;
                log!("i: {i} j: {j} v: {v:08X} o: {o:08X}");
                self.data[j].0 = o << lx & m | v & mi;
            }
        }
        if ly != 0 {
            let s = ly * 8;
            let m = u32::MAX << s;
            let mi = !m;
            for i in 0..sx {
                let j = endrow + i;
                let v = self.data[j].0;
                let o = self.data[i].0;
                log!("i: {i} j: {j} v: {v:08X} o: {o:08X}");
                self.data[j].0 = o << s & m | v & mi;
            }
        }

        for i in 0..sy {
            let endy = i == sy - 1;
            let i = i * sx;

            for j in 0..sx {
                let ix = i + j;
                let endx = j == sx - 1;
                log!("i: {i} j: {j}");

                let v = to_vec(self.data[ix].0);
                let vl = u32x4_shuffle::<4, 0, 1, 2>(v, u32x4_splat(0));
                let vh = u32x4_shuffle::<1, 2, 3, 4>(v, u32x4_splat(0));
                let mut r = u8x16_add(vl, vh);
                r = [
                    u32x4_shl(r, 4),
                    u32x4_shr(r, 4),
                    u32x4_shl(v, 4),
                    u32x4_shr(v, 4),
                ]
                .into_iter()
                .fold(r, _u8x16_add);
                log!("v: {:032X} r: {:032X}", print_v128(v), print_v128(r));

                let o = if j == 0 {
                    log!("ix: {}", sx - 1 + i);
                    self.data[sx - 1 + i].0.wrapping_shl(((8 - lx) & 7) as _)
                } else {
                    log!("ix: {}", ix - 1);
                    self.data[ix - 1].0
                };
                let o = to_vec(o >> 7 & 0x01010101);
                r = [
                    o,
                    u32x4_shuffle::<4, 0, 1, 2>(o, u32x4_splat(0)),
                    u32x4_shuffle::<1, 2, 3, 4>(o, u32x4_splat(0)),
                ]
                .into_iter()
                .fold(r, _u8x16_add);
                log!("o: {:032X} r: {:032X}", print_v128(o), print_v128(r));

                if !endx || lx == 0 {
                    let ix_ = if endx { i } else { ix + 1 };
                    let o = to_vec(self.data[ix_].0 << 7 & 0x80808080);
                    r = [
                        o,
                        u32x4_shuffle::<4, 0, 1, 2>(o, u32x4_splat(0)),
                        u32x4_shuffle::<1, 2, 3, 4>(o, u32x4_splat(0)),
                    ]
                    .into_iter()
                    .fold(r, _u8x16_add);
                    log!(
                        "ix: {ix_} o: {:032X} r: {:032X}",
                        print_v128(o),
                        print_v128(r)
                    );

                    let o = if i == 0 {
                        let ix = if endx { endrow } else { endrow + j + 1 };
                        log!("ix: {ix}");
                        self.data[ix].0.wrapping_shl(((4 - ly) * 8) as _)
                    } else {
                        let ix = if endx { i - sx } else { ix - sx + 1 };
                        log!("ix: {ix}");
                        self.data[ix].0
                    };
                    log!("o: {o:08X}");
                    if o & 0x100_0000 != 0 {
                        r = u8x16_add(r, u8x16_replace_lane::<3>(u8x16_splat(0), 16));
                    }
                }

                let o = if i == 0 {
                    log!("ix: {}", endrow + j);
                    self.data[endrow + j].0.wrapping_shl(((4 - ly) * 8) as _)
                } else {
                    log!("ix: {}", ix - sx);
                    self.data[ix - sx].0
                };
                let o = to_vec(o >> 24);
                r = [o, u32x4_shl(o, 4), u32x4_shr(o, 4)]
                    .into_iter()
                    .fold(r, _u8x16_add);
                log!("o: {:032X} r: {:032X}", print_v128(o), print_v128(r));

                if !endy || ly == 0 {
                    let ix_ = if endy { j } else { ix + sx };
                    let o = to_vec(self.data[ix_].0 << 24);
                    r = [o, u32x4_shl(o, 4), u32x4_shr(o, 4)]
                        .into_iter()
                        .fold(r, _u8x16_add);
                    log!(
                        "ix: {ix_} o: {:032X} r: {:032X}",
                        print_v128(o),
                        print_v128(r)
                    );

                    let o = if j == 0 {
                        let ix = if endy { sx - 1 } else { sx * 2 - 1 + i };
                        log!("ix: {ix}");
                        self.data[ix].0.wrapping_shl(((8 - lx) & 7) as _)
                    } else {
                        let ix = if endy { j - 1 } else { sx - 1 + ix };
                        log!("ix: {ix}");
                        self.data[ix].0
                    };
                    log!("o: {o:08X}");
                    if o & 0x80 != 0 {
                        r = u8x16_add(r, u8x16_replace_lane::<12>(u8x16_splat(0), 1));
                    }
                }

                let o = if i == 0 {
                    let v = if j == 0 {
                        self.data[self.data.len() - 1]
                            .0
                            .wrapping_shl(((8 - lx) & 7) as _)
                    } else {
                        self.data[endrow + j - 1].0
                    };
                    v.wrapping_shl(((4 - ly) * 8) as _)
                } else if j == 0 {
                    self.data[i - 1].0.wrapping_shl(((8 - lx) & 7) as _)
                } else {
                    self.data[ix - sx - 1].0
                };
                if o & 0x8000_0000 != 0 {
                    r = u8x16_add(r, u8x16_replace_lane::<0>(u8x16_splat(0), 1));
                }

                if (!endx || lx == 0) && (!endy || ly == 0) {
                    let ix = match (endx, endy) {
                        (false, false) => ix + sx + 1,
                        (false, true) => j + 1,
                        (true, false) => i + sx,
                        (true, true) => 0,
                    };
                    if self.data[ix].0 & 1 != 0 {
                        r = u8x16_add(r, u8x16_replace_lane::<15>(u8x16_splat(0), 16));
                    }
                }

                self.data[ix].1 = from_vec(apply_rule(v, r));
            }
        }

        for (i, j) in &mut self.data {
            *i = *j;
        }
    }

    fn click(&mut self, x: f32, y: f32, right_click: bool) {
        if right_click {
            self.paused = !self.paused;
            return;
        }

        let (x, y) = ((x / 4.) as i32, (y / 4.) as i32);
        let r = 0..self.size as i32;
        if !r.contains(&x) || !r.contains(&y) {
            return;
        }

        let (x, y) = (x as usize, y as usize);
        let i = (x >> 3) + (y >> 2) * ((self.size + 7) >> 3);
        let b = x & 7 | (y & 3) << 3;
        self.data[i].0 ^= 1 << b;
    }

    fn render(&self, state: &mut State) {
        state.resize(self.size * 4, self.size * 4);

        static PATTERN: &[(u32, bool)] = &[
            (0, true),
            (0, false),
            (0, false),
            (0, true),
            (8, false),
            (8, false),
            (8, false),
            (8, true),
            (16, false),
            (16, false),
            (16, false),
            (16, true),
            (24, false),
            (24, false),
            (24, false),
            (24, true),
        ];

        for (c, (a, &(o, h))) in state.colors_mut().chunks_exact_mut(self.size * 4).zip(
            self.data
                .chunks_exact((self.size + 7) >> 3)
                .flat_map(|a| repeat(a).zip(PATTERN)),
        ) {
            for (c, (mut v, _)) in c.chunks_mut(32).zip(a) {
                let mut h = if h { u32::MAX } else { 0x8888_8889 };
                v >>= o;
                for i in c.chunks_exact_mut(4) {
                    let b = v & 1 != 0;
                    for i in i {
                        let c = match (b, h & 1 != 0) {
                            (false, false) => 0,
                            (false, true) => 64,
                            (true, false) => 255,
                            (true, true) => 191,
                        };
                        *i = Color {
                            r: c,
                            g: c,
                            b: c,
                            a: 255,
                        };
                        h >>= 1;
                    }
                    v >>= 1;
                }
            }
        }
    }
}

fn print_v128(v: v128) -> u128 {
    u64x2_extract_lane::<0>(v) as u128 | (u64x2_extract_lane::<1>(v) as u128) << 64
}

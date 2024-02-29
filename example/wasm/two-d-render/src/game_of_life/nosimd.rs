use std::iter::repeat;

use super::SIZE;
use crate::{log, Color, Renderable, State};

#[derive(Debug, Default)]
pub struct GameOfLife {
    size: usize,
    data: Vec<(u16, u16)>,
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
    let mut ret = 0u64;
    for mut s in 0..16 {
        s *= 4;
        ret |= match r >> s & 15 {
            0..=1 => 0,
            2 => v & 1 << s,
            3 => 1 << s,
            4.. => 0,
        };
    }
    ret
}

impl Renderable for GameOfLife {
    fn new() -> Self {
        let data = vec![(0, 0); ((SIZE + 3) >> 2) * ((SIZE + 3) >> 2)];

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

        let sx = (self.size + 3) >> 2;
        let endrow = sx * (sx - 1);
        debug_assert_eq!(self.data.len(), sx * sx);

        log!("sx: {sx}");

        let lx = self.size & 3;
        log!("lx: {lx}");

        if lx != 0 {
            let mut m = ((15u8 << lx) & 15) as u16;
            m = m | m << 4 | m << 8 | m << 12;
            let mi = !m;
            for i in 0..sx {
                let j = endrow + i;
                let v = self.data[j].0;
                let o = self.data[i].0;
                log!("i: {i} j: {j} v: {v:04X} o: {o:04X}");
                self.data[j].0 = o << lx & m | v & mi;
            }
            let s = lx * 4;
            let m = u16::MAX << s;
            let mi = !m;
            for i in 0..sx {
                let j = endrow + i;
                let v = self.data[j].0;
                let o = self.data[i].0;
                log!("i: {i} j: {j} v: {v:04X} o: {o:04X}");
                self.data[j].0 = o << s & m | v & mi;
            }
        }

        for i in 0..sx {
            let endy = i == sx - 1;
            let i = i * sx;

            for j in 0..sx {
                let ix = i + j;
                let endx = j == sx - 1;
                log!("i: {i} j: {j}");

                let v = to_vec(self.data[ix].0);
                let vl = v >> 4 & 0x0111_0111_0111_0111;
                let vr = v << 4 & 0x1110_1110_1110_1110;
                let mut r = vl + vr;
                r += [r << 16, r >> 16, v << 16, v >> 16]
                    .into_iter()
                    .sum::<u64>();
                log!("v: {v:016X} r: {r:016X}");

                let o = if j == 0 {
                    log!("ix: {}", sx - 1 + i);
                    self.data[sx - 1 + i].0.wrapping_shl(((4 - lx) * 4) as _)
                } else {
                    log!("ix: {}", ix - 1);
                    self.data[ix - 1].0
                };
                let o = to_vec(o >> 12);
                r += [o, o << 16, o >> 16].into_iter().sum::<u64>();
                log!("o: {o:016X} r: {r:016X}");

                if !endx || lx == 0 {
                    let ix_ = if endx { i } else { ix + 1 };
                    let o = to_vec(self.data[ix_].0 << 12);
                    r += [o, o << 16, o >> 16].into_iter().sum::<u64>();
                    log!("ix: {ix_} o: {o:016X} r: {r:016X}");

                    let o = if i == 0 {
                        let ix = if endx { endrow } else { endrow + j + 1 };
                        log!("ix: {ix}");
                        self.data[ix].0.wrapping_shl(((4 - lx) & 3) as _)
                    } else {
                        let ix = if endx { i - sx } else { ix - sx + 1 };
                        log!("ix: {ix}");
                        self.data[ix].0
                    };
                    log!("o: {o:04X}");
                    if o & 8 != 0 {
                        r += 0x1000;
                    }
                }

                let o = if i == 0 {
                    log!("ix: {}", endrow + j);
                    self.data[endrow + j].0.wrapping_shl(((4 - lx) & 3) as _)
                } else {
                    log!("ix: {}", ix - sx);
                    self.data[ix - sx].0
                };
                let o = to_vec(o >> 3 & 0x1111);
                r += [
                    o,
                    o << 4 & 0x1110_1110_1110_1110,
                    o >> 4 & 0x0111_0111_0111_0111,
                ]
                .into_iter()
                .sum::<u64>();
                log!("o: {o:016X} r: {r:016X}");

                if !endy || lx == 0 {
                    let ix_ = if endy { j } else { ix + sx };
                    let o = to_vec(self.data[ix_].0 << 3 & 0x8888);
                    r += [
                        o,
                        o << 4 & 0x1110_1110_1110_1110,
                        o >> 4 & 0x0111_0111_0111_0111,
                    ]
                    .into_iter()
                    .sum::<u64>();
                    log!("ix: {ix_} o: {o:016X} r: {r:016X}");

                    let o = if j == 0 {
                        let ix = if endy { sx - 1 } else { sx * 2 - 1 + i };
                        log!("ix: {ix}");
                        self.data[ix].0.wrapping_shl(((4 - lx) * 4) as _)
                    } else {
                        let ix = if endy { j - 1 } else { sx - 1 + ix };
                        log!("ix: {ix}");
                        self.data[ix].0
                    };
                    log!("o: {o:04X}");
                    if o & 0x1000 != 0 {
                        r += 0x1_0000_0000_0000;
                    }
                }

                let o = if i == 0 {
                    let v = if j == 0 {
                        self.data[self.data.len() - 1]
                            .0
                            .wrapping_shl(((4 - lx) * 4) as _)
                    } else {
                        self.data[endrow + j - 1].0
                    };
                    v.wrapping_shl(((4 - lx) & 3) as _)
                } else if j == 0 {
                    self.data[i - 1].0.wrapping_shl(((4 - lx) * 4) as _)
                } else {
                    self.data[ix - sx - 1].0
                };
                if o & 0x8000 != 0 {
                    r += 1;
                }

                if lx == 0 || !endx && !endy {
                    let ix = match (endx, endy) {
                        (false, false) => ix + sx + 1,
                        (false, true) => j + 1,
                        (true, false) => i + sx,
                        (true, true) => 0,
                    };
                    if self.data[ix].0 & 1 != 0 {
                        r += 0x1000_0000_0000_0000;
                    }
                }

                let o = apply_rule(v, r);
                log!("v: {v:016X} r: {r:016X} o: {o:016X}");
                self.data[ix].1 = from_vec(o);
                log!("output: {:04X}", self.data[ix].1);
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
        let i = (x >> 2) + (y >> 2) * ((self.size + 3) >> 2);
        let b = y & 3 | (x & 3) << 2;
        self.data[i].0 ^= 1 << b;
    }

    fn render(&self, state: &mut State) {
        state.resize(self.size * 4, self.size * 4);

        static PATTERN: &[(u32, bool)] = &[
            (0, true),
            (0, false),
            (0, false),
            (0, true),
            (1, false),
            (1, false),
            (1, false),
            (1, true),
            (2, false),
            (2, false),
            (2, false),
            (2, true),
            (3, false),
            (3, false),
            (3, false),
            (3, true),
        ];

        for (c, (a, &(o, h))) in state.colors_mut().chunks_exact_mut(self.size * 4).zip(
            self.data
                .chunks_exact((self.size + 3) >> 2)
                .flat_map(|a| repeat(a).zip(PATTERN)),
        ) {
            for (c, (mut v, _)) in c.chunks_mut(16).zip(a) {
                let mut h = if h { u16::MAX } else { 0x8889 };
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
                    v >>= 4;
                }
            }
        }
    }
}

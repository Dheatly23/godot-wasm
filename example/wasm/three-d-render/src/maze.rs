use std::ops::Mul;

use glam::bool::*;
use glam::f32::*;
use rand::prelude::*;
use rand_xoshiro::Xoshiro512StarStar;

use crate::{log, Color, MouseButton, Renderable, State};

const SIZE: usize = 8;
const TIME_SCALE: f32 = 1. / 64.;
const SPACE_SCALE: f32 = 5. / (SIZE as f32);
const SPACE_OFFSET: Vec3 = Vec3::splat(-2.5 + 0.5 * SPACE_SCALE);
const MAX_REP: usize = 16;

#[derive(Debug)]
struct Candidate {
    x: usize,
    y: usize,
    z: usize,
    next: Option<Box<Candidate>>,
}

const FLAG_R: u8 = 1;
const FLAG_U: u8 = 2;
const FLAG_F: u8 = 4;
const FLAG_C: u8 = 64;
const FLAG_M: u8 = 128;

#[derive(Debug, Clone, Copy)]
enum Slice {
    NoSlice,
    X(usize),
    Y(usize),
    Z(usize),
}

#[derive(Debug)]
pub struct Maze {
    data: Vec<u8>,
    candidate: Option<Box<Candidate>>,
    len: usize,

    rng: Xoshiro512StarStar,

    residue: f32,
    paused: bool,

    slice: Slice,
}

impl Renderable for Maze {
    fn new() -> Self {
        let mut ret = Self {
            data: vec![0; SIZE * SIZE * SIZE],
            candidate: None,
            len: 0,

            rng: Xoshiro512StarStar::from_os_rng(),

            residue: 0.0,
            paused: false,

            slice: Slice::NoSlice,
        };

        let x = ret.rng.random_range(0..SIZE);
        let y = ret.rng.random_range(0..SIZE);
        let z = ret.rng.random_range(0..SIZE);
        *ret.data_mut(x, y, z) |= FLAG_M;
        ret.add_candidates(x, y, z);

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
            if self.len == 0 {
                break;
            }
            let i = self.rng.random_range(0..self.len);
            let mut cp = &mut self.candidate;
            for _ in 0..i {
                cp = &mut cp.as_mut().unwrap().next;
            }

            let mut c = cp.take().unwrap();
            *cp = c.next.take();
            self.len -= 1;
            let Candidate { x, y, z, .. } = *c;
            drop(c);

            *self.data_mut(x, y, z) |= FLAG_M;
            self.connect_candidate(x, y, z);
            self.add_candidates(x, y, z);
        }
    }

    fn click(&mut self, origin: Vec3, norm: Vec3, button: MouseButton) {
        if let MouseButton::Right = button {
            self.paused = !self.paused;
        } else if let MouseButton::Middle = button {
            self.data.fill(0);

            let x = self.rng.random_range(0..SIZE);
            let y = self.rng.random_range(0..SIZE);
            let z = self.rng.random_range(0..SIZE);
            *self.data_mut(x, y, z) |= FLAG_M;
            self.add_candidates(x, y, z);
        } else if let MouseButton::Left = button {
            log!("o: {origin} n: {norm}");
            let (so, ss) = (Vec3A::from(SPACE_OFFSET), Vec3A::splat(SPACE_SCALE));
            let mut tm = (Vec3A::from(SPACE_OFFSET) - Vec3A::from(origin)) / Vec3A::from(norm);
            let Mat3A {
                x_axis: txm,
                y_axis: tym,
                z_axis: tzm,
            } = Mat3A::from_cols(
                (tm * norm.x + origin.x - so) / ss,
                (tm * norm.y + origin.y - so) / ss,
                (tm * norm.z + origin.z - so) / ss,
            )
            .transpose();
            log!("tm: {tm} txm: {txm} tym: {tym} tzm: {tzm}");
            let mut tp = (Vec3A::from(SPACE_OFFSET + SPACE_SCALE * (SIZE as f32 - 0.5))
                - Vec3A::from(origin))
                / Vec3A::from(norm);
            let Mat3A {
                x_axis: txp,
                y_axis: typ,
                z_axis: tzp,
            } = Mat3A::from_cols(
                (tp * norm.x + origin.x - so) / ss,
                (tp * norm.y + origin.y - so) / ss,
                (tp * norm.z + origin.z - so) / ss,
            )
            .transpose();
            log!("tp: {tp} txp: {txp} typ: {typ} tzp: {tzp}");

            tm = Vec3A::select(
                tm.cmplt(Vec3A::ZERO)
                    | BVec3A::new(
                        (txm.cmplt(Vec3A::ZERO) | txm.cmpge(Vec3A::splat(SIZE as f32))).bitmask()
                            & 0b110
                            != 0,
                        (tym.cmplt(Vec3A::ZERO) | tym.cmpge(Vec3A::splat(SIZE as f32))).bitmask()
                            & 0b101
                            != 0,
                        (tzm.cmplt(Vec3A::ZERO) | tzm.cmpge(Vec3A::splat(SIZE as f32))).bitmask()
                            & 0b011
                            != 0,
                    ),
                Vec3A::INFINITY,
                tm,
            );
            tp = Vec3A::select(
                tp.cmplt(Vec3A::ZERO)
                    | BVec3A::new(
                        (txp.cmplt(Vec3A::ZERO) | txp.cmpge(Vec3A::splat(SIZE as f32))).bitmask()
                            & 0b110
                            != 0,
                        (typ.cmplt(Vec3A::ZERO) | typ.cmpge(Vec3A::splat(SIZE as f32))).bitmask()
                            & 0b101
                            != 0,
                        (tzp.cmplt(Vec3A::ZERO) | tzp.cmpge(Vec3A::splat(SIZE as f32))).bitmask()
                            & 0b011
                            != 0,
                    ),
                Vec3A::INFINITY,
                tp,
            );

            log!("tm: {tm} tp: {tp}");
            let m = tm.min(tp).min_element();
            if !m.is_finite() {
                return;
            }
            let m_ = Vec3A::splat(m);
            let b = tm.cmpeq(m_).bitmask() << 3 | tp.cmpeq(m_).bitmask();
            log!("{b:08b}");

            fn vec_to_uarr(v: Vec3A) -> [usize; 3] {
                let [x, y, z] = v.to_array();
                [x as _, y as _, z as _]
            }

            self.slice = match b.trailing_zeros() {
                v @ (0 | 3) => {
                    let [_, y, z] = vec_to_uarr(if v == 0 { txp } else { txm });
                    match self.slice {
                        Slice::Z(v) if v == z => Slice::Y(y),
                        Slice::Y(v) if v == y => Slice::NoSlice,
                        Slice::Y(_) => Slice::Y(y),
                        _ => Slice::Z(z),
                    }
                }
                v @ (1 | 4) => {
                    let [x, _, z] = vec_to_uarr(if v == 1 { typ } else { tym });
                    match self.slice {
                        Slice::X(v) if v == x => Slice::Z(z),
                        Slice::Z(v) if v == z => Slice::NoSlice,
                        Slice::Z(_) => Slice::Z(z),
                        _ => Slice::X(x),
                    }
                }
                v @ (2 | 5) => {
                    let [x, y, _] = vec_to_uarr(if v == 2 { tzp } else { tzm });
                    match self.slice {
                        Slice::X(v) if v == x => Slice::Y(y),
                        Slice::Y(v) if v == y => Slice::NoSlice,
                        Slice::Y(_) => Slice::Y(y),
                        _ => Slice::X(x),
                    }
                }
                _ => return,
            };
            log!("{:?}", self.slice);
        }
    }

    fn render(&self, state: &mut State) {
        state.vertex.clear();
        state.normal.clear();
        state.tangent.clear();
        state.uv.clear();
        state.color.clear();
        state.index.clear();

        const CAND_C: Color = Color {
            r: 0.5,
            g: 0.5,
            b: 0.0,
            a: 1.0,
        };
        const SIDE_C: Color = Color {
            r: 1.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        const TUBE_C: Color = Color {
            r: 0.0,
            g: 0.0,
            b: 1.0,
            a: 1.0,
        };

        let mut i = 0;
        for z in 0..SIZE {
            let z_ = z as f32 * SPACE_SCALE;
            for y in 0..SIZE {
                let y_ = y as f32 * SPACE_SCALE;
                for x in 0..SIZE {
                    let x_ = x as f32 * SPACE_SCALE;
                    let j = i;
                    i += 1;
                    let Some(&v) = self.data.get(j) else {
                        return;
                    };
                    if v & (FLAG_C | FLAG_M) == FLAG_C {
                        match self.slice {
                            Slice::X(v) if x.abs_diff(v) > 1 => continue,
                            Slice::Y(v) if y.abs_diff(v) > 1 => continue,
                            Slice::Z(v) if z.abs_diff(v) > 1 => continue,
                            _ => (),
                        }

                        // L
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_, z_) + SPACE_OFFSET,
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., SPACE_SCALE / 2., 0.),
                            Vec3::new(-1., 0., 0.),
                            Vec4::new(0., 0., -1., 1.),
                            CAND_C,
                            UVPos::FlipX,
                            true,
                        );
                        // R
                        add_quad(
                            &mut *state,
                            Vec3::new(x_ + SPACE_SCALE / 2., y_, z_) + SPACE_OFFSET,
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., SPACE_SCALE / 2., 0.),
                            Vec3::new(1., 0., 0.),
                            Vec4::new(0., 0., 1., 1.),
                            CAND_C,
                            UVPos::None,
                            false,
                        );
                        // D
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_, z_) + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., -1., 0.),
                            Vec4::new(-1., 0., 0., 1.),
                            CAND_C,
                            UVPos::FlipBoth,
                            true,
                        );
                        // U
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_ + SPACE_SCALE / 2., z_) + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., 1., 0.),
                            Vec4::new(0., 0., 1., 1.),
                            CAND_C,
                            UVPos::None,
                            false,
                        );
                        // B
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_, z_) + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., SPACE_SCALE / 2., 0.),
                            Vec3::new(0., 0., -1.),
                            Vec4::new(1., 0., 0., 1.),
                            CAND_C,
                            UVPos::None,
                            false,
                        );
                        // F
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_, z_ + SPACE_SCALE / 2.) + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., SPACE_SCALE / 2., 0.),
                            Vec3::new(0., 0., 1.),
                            Vec4::new(-1., 0., 0., 1.),
                            CAND_C,
                            UVPos::FlipX,
                            true,
                        );
                    }
                    if v & FLAG_M == 0 {
                        continue;
                    }

                    #[derive(Clone, Copy, PartialEq, Eq)]
                    enum Axis {
                        NoAxis,
                        X,
                        Y,
                        Z,
                    }

                    let axis = match self.slice {
                        Slice::X(v @ 1..) if x == v - 1 => Axis::X,
                        Slice::Y(v @ 1..) if y == v - 1 => Axis::Y,
                        Slice::Z(v @ 1..) if z == v - 1 => Axis::Z,
                        Slice::X(v) if x != v => continue,
                        Slice::Y(v) if y != v => continue,
                        Slice::Z(v) if z != v => continue,
                        _ => Axis::NoAxis,
                    };

                    if (x == 0 || self.data[j - 1] & FLAG_R == 0) && axis == Axis::NoAxis {
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_, z_) + SPACE_OFFSET,
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., SPACE_SCALE / 2., 0.),
                            Vec3::new(-1., 0., 0.),
                            Vec4::new(0., 0., -1., 1.),
                            SIDE_C,
                            UVPos::FlipX,
                            true,
                        );
                    }
                    if v & FLAG_R != 0 && matches!(axis, Axis::NoAxis | Axis::X) {
                        add_tube(
                            &mut *state,
                            Vec3::new(x_ + SPACE_SCALE / 2., y_ + SPACE_SCALE / 2., z_)
                                + SPACE_OFFSET,
                            Vec3::new(0., -SPACE_SCALE / 2., 0.),
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., -1., 0.),
                            Vec3::new(1., 0., 0.),
                            Vec3::new(0., 0., 1.),
                            TUBE_C,
                            UVPos::None,
                        );
                    } else if axis == Axis::NoAxis {
                        add_quad(
                            &mut *state,
                            Vec3::new(x_ + SPACE_SCALE / 2., y_, z_) + SPACE_OFFSET,
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., SPACE_SCALE / 2., 0.),
                            Vec3::new(1., 0., 0.),
                            Vec4::new(0., 0., 1., 1.),
                            SIDE_C,
                            UVPos::None,
                            false,
                        );
                    }

                    if (y == 0 || self.data[j - SIZE] & FLAG_U == 0) && axis == Axis::NoAxis {
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_, z_) + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., -1., 0.),
                            Vec4::new(-1., 0., 0., 1.),
                            SIDE_C,
                            UVPos::FlipBoth,
                            true,
                        );
                    }
                    if v & FLAG_U != 0 && matches!(axis, Axis::NoAxis | Axis::Y) {
                        add_tube(
                            &mut *state,
                            Vec3::new(x_, y_ + SPACE_SCALE / 2., z_) + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., SPACE_SCALE / 2., 0.),
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(1., 0., 0.),
                            Vec3::new(0., 1., 0.),
                            Vec3::new(0., 0., 1.),
                            TUBE_C,
                            UVPos::None,
                        );
                    } else if axis == Axis::NoAxis {
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_ + SPACE_SCALE / 2., z_) + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., 1., 0.),
                            Vec4::new(0., 0., 1., 1.),
                            SIDE_C,
                            UVPos::None,
                            false,
                        );
                    }

                    if (z == 0 || self.data[j - SIZE * SIZE] & FLAG_F == 0) && axis == Axis::NoAxis
                    {
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_, z_) + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., SPACE_SCALE / 2., 0.),
                            Vec3::new(0., 0., -1.),
                            Vec4::new(1., 0., 0., 1.),
                            SIDE_C,
                            UVPos::None,
                            false,
                        );
                    }
                    if v & FLAG_F != 0 && matches!(axis, Axis::NoAxis | Axis::Z) {
                        add_tube(
                            &mut *state,
                            Vec3::new(x_, y_ + SPACE_SCALE / 2., z_ + SPACE_SCALE / 2.)
                                + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., 0., SPACE_SCALE / 2.),
                            Vec3::new(0., -SPACE_SCALE / 2., 0.),
                            Vec3::new(1., 0., 0.),
                            Vec3::new(0., 0., 1.),
                            Vec3::new(0., -1., 0.),
                            TUBE_C,
                            UVPos::None,
                        );
                    } else if axis == Axis::NoAxis {
                        add_quad(
                            &mut *state,
                            Vec3::new(x_, y_, z_ + SPACE_SCALE / 2.) + SPACE_OFFSET,
                            Vec3::new(SPACE_SCALE / 2., 0., 0.),
                            Vec3::new(0., SPACE_SCALE / 2., 0.),
                            Vec3::new(0., 0., 1.),
                            Vec4::new(-1., 0., 0., 1.),
                            SIDE_C,
                            UVPos::FlipX,
                            true,
                        );
                    }
                }
            }
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq)]
enum UVPos {
    None = 0,
    FlipX = 1,
    FlipY = 2,
    FlipBoth = 3,
    Swap = 4,
    SwapFlipX = 5,
    SwapFlipY = 6,
    SwapFlipBoth = 7,
}

impl Mul for UVPos {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let (a, mut b) = (self as u8, rhs as u8);
        if a & 4 != 0 {
            b = ((b & 4) ^ 4) | (b & 1) << 1 | (b & 2) >> 1;
        }
        b ^= a & 3;
        match b {
            0 => Self::None,
            1 => Self::FlipX,
            2 => Self::FlipY,
            3 => Self::FlipBoth,
            4 => Self::Swap,
            5 => Self::SwapFlipX,
            6 => Self::SwapFlipY,
            7 => Self::SwapFlipBoth,
            _ => unreachable!(),
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn add_quad(
    state: &mut State,
    o: Vec3,
    dx: Vec3,
    dy: Vec3,
    n: Vec3,
    t: Vec4,
    c: Color,
    uv: UVPos,
    flip: bool,
) {
    let ix = state.vertex.len() as u32;
    state.vertex.extend([o, o + dx, o + dy, o + dx + dy]);
    state.normal.extend([n; 4]);
    state.tangent.extend([t; 4]);
    state.uv.extend(match uv {
        UVPos::None => [
            Vec2::new(0., 0.),
            Vec2::new(1., 0.),
            Vec2::new(0., 1.),
            Vec2::new(1., 1.),
        ],
        UVPos::FlipX => [
            Vec2::new(1., 0.),
            Vec2::new(0., 0.),
            Vec2::new(1., 1.),
            Vec2::new(0., 1.),
        ],
        UVPos::FlipY => [
            Vec2::new(0., 1.),
            Vec2::new(1., 1.),
            Vec2::new(0., 0.),
            Vec2::new(1., 0.),
        ],
        UVPos::FlipBoth => [
            Vec2::new(1., 1.),
            Vec2::new(0., 1.),
            Vec2::new(1., 0.),
            Vec2::new(0., 0.),
        ],
        UVPos::Swap => [
            Vec2::new(0., 0.),
            Vec2::new(0., 1.),
            Vec2::new(1., 0.),
            Vec2::new(1., 1.),
        ],
        UVPos::SwapFlipX => [
            Vec2::new(0., 1.),
            Vec2::new(0., 0.),
            Vec2::new(1., 1.),
            Vec2::new(1., 0.),
        ],
        UVPos::SwapFlipY => [
            Vec2::new(1., 0.),
            Vec2::new(1., 1.),
            Vec2::new(0., 0.),
            Vec2::new(0., 1.),
        ],
        UVPos::SwapFlipBoth => [
            Vec2::new(1., 1.),
            Vec2::new(1., 0.),
            Vec2::new(0., 1.),
            Vec2::new(0., 0.),
        ],
    });
    state.color.extend([c; 4]);
    state.index.extend(if flip {
        [ix, ix + 3, ix + 1, ix, ix + 2, ix + 3]
    } else {
        [ix, ix + 1, ix + 3, ix, ix + 3, ix + 2]
    });
}

#[allow(clippy::too_many_arguments)]
#[inline]
fn add_tube(
    state: &mut State,
    o: Vec3,
    dx: Vec3,
    dy: Vec3,
    dz: Vec3,
    nx: Vec3,
    _ny: Vec3,
    nz: Vec3,
    c: Color,
    uv: UVPos,
) {
    add_quad(
        &mut *state,
        o,
        dz,
        dy,
        -nx,
        (-nz).extend(1.),
        c,
        uv * UVPos::FlipX,
        true,
    );
    add_quad(
        &mut *state,
        o + dx,
        dz,
        dy,
        nx,
        nz.extend(1.),
        c,
        uv * UVPos::None,
        false,
    );
    add_quad(
        &mut *state,
        o,
        dx,
        dy,
        -nz,
        nx.extend(1.),
        c,
        uv * UVPos::None,
        false,
    );
    add_quad(
        &mut *state,
        o + dz,
        dx,
        dy,
        nz,
        (-nx).extend(1.),
        c,
        uv * UVPos::FlipX,
        true,
    );
}

impl Maze {
    fn data(&self, x: usize, y: usize, z: usize) -> &u8 {
        &self.data[x + (y + z * SIZE) * SIZE]
    }

    fn data_mut(&mut self, x: usize, y: usize, z: usize) -> &mut u8 {
        &mut self.data[x + (y + z * SIZE) * SIZE]
    }

    fn add_candidates(&mut self, x: usize, y: usize, z: usize) {
        if let Some(x) = x.checked_sub(1) {
            self.push_candidate(x, y, z);
        }
        if x + 1 < SIZE {
            self.push_candidate(x + 1, y, z);
        }
        if let Some(y) = y.checked_sub(1) {
            self.push_candidate(x, y, z);
        }
        if y + 1 < SIZE {
            self.push_candidate(x, y + 1, z);
        }
        if let Some(z) = z.checked_sub(1) {
            self.push_candidate(x, y, z);
        }
        if z + 1 < SIZE {
            self.push_candidate(x, y, z + 1);
        }
    }

    fn push_candidate(&mut self, x: usize, y: usize, z: usize) {
        let p = self.data_mut(x, y, z);
        if *p & (FLAG_M | FLAG_C) != 0 {
            return;
        }
        *p |= FLAG_C;
        self.candidate = Some(Box::new(Candidate {
            x,
            y,
            z,
            next: self.candidate.take(),
        }));
        self.len += 1;
    }

    fn connect_candidate(&mut self, x: usize, y: usize, z: usize) {
        #[derive(Clone, Copy)]
        enum Dir {
            L,
            R,
            U,
            D,
            F,
            B,
        }

        let mut s = [
            if x > 0 && *self.data(x - 1, y, z) & FLAG_M != 0 {
                Some(Dir::L)
            } else {
                None
            },
            if y > 0 && *self.data(x, y - 1, z) & FLAG_M != 0 {
                Some(Dir::D)
            } else {
                None
            },
            if z > 0 && *self.data(x, y, z - 1) & FLAG_M != 0 {
                Some(Dir::B)
            } else {
                None
            },
            if x < SIZE - 1 && *self.data(x + 1, y, z) & FLAG_M != 0 {
                Some(Dir::R)
            } else {
                None
            },
            if y < SIZE - 1 && *self.data(x, y + 1, z) & FLAG_M != 0 {
                Some(Dir::U)
            } else {
                None
            },
            if z < SIZE - 1 && *self.data(x, y, z + 1) & FLAG_M != 0 {
                Some(Dir::F)
            } else {
                None
            },
        ];
        let s_ = {
            let mut i = 0;
            let mut j = s.len() - 1;
            loop {
                while i < s.len() && s[i].is_some() {
                    i += 1;
                }
                while j > 0 && s[j].is_none() {
                    j -= 1;
                }
                if i >= j {
                    break &s[..i];
                }
                s.swap(i, j);
                i += 1;
                j -= 1;
            }
        };

        match s_.choose(&mut self.rng).copied().flatten() {
            None => (),
            Some(Dir::L) => *self.data_mut(x - 1, y, z) |= FLAG_R,
            Some(Dir::D) => *self.data_mut(x, y - 1, z) |= FLAG_U,
            Some(Dir::B) => *self.data_mut(x, y, z - 1) |= FLAG_F,
            Some(Dir::R) => *self.data_mut(x, y, z) |= FLAG_R,
            Some(Dir::U) => *self.data_mut(x, y, z) |= FLAG_U,
            Some(Dir::F) => *self.data_mut(x, y, z) |= FLAG_F,
        }
    }
}

mod double_joint;
mod wave;

use std::ptr::{addr_of, addr_of_mut, null};

use glam::f32::*;

trait Renderable {
    fn new() -> Self;
    fn render(&self, state: &mut State);
    fn step(&mut self, time: f32, delta: f32);
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

#[derive(Default)]
pub(crate) struct State {
    pub(crate) vertex: Vec<Vec3>,
    pub(crate) normal: Vec<Vec3>,
    pub(crate) tangent: Vec<Vec4>,
    pub(crate) uv: Vec<Vec2>,
    pub(crate) color: Vec<Color>,
    pub(crate) index: Vec<u32>,
}

#[repr(C)]
pub struct ExportState {
    pub vertex_ptr: *const Vec3,
    pub vertex_cnt: usize,
    pub normal_ptr: *const Vec3,
    pub normal_cnt: usize,
    pub tangent_ptr: *const Vec4,
    pub tangent_cnt: usize,
    pub uv_ptr: *const Vec2,
    pub uv_cnt: usize,
    pub color_ptr: *const Color,
    pub color_cnt: usize,
    pub index_ptr: *const u32,
    pub index_cnt: usize,
}

enum RenderData {
    Wave(wave::Wave),
    DoubleJoint(double_joint::DoubleJoint),
}

impl RenderData {
    fn render(&self, state: &mut State) {
        match self {
            Self::Wave(v) => v.render(state),
            Self::DoubleJoint(v) => v.render(state),
        }
    }

    fn step(&mut self, time: f32, delta: f32) {
        match self {
            Self::Wave(v) => v.step(time, delta),
            Self::DoubleJoint(v) => v.step(time, delta),
        }
    }
}

static mut RENDER: Option<RenderData> = None;
static mut STATE: State = State {
    vertex: Vec::new(),
    normal: Vec::new(),
    tangent: Vec::new(),
    uv: Vec::new(),
    color: Vec::new(),
    index: Vec::new(),
};
static mut STATE_EXPORT: ExportState = ExportState {
    vertex_ptr: null(),
    vertex_cnt: 0,
    normal_ptr: null(),
    normal_cnt: 0,
    tangent_ptr: null(),
    tangent_cnt: 0,
    uv_ptr: null(),
    uv_cnt: 0,
    color_ptr: null(),
    color_cnt: 0,
    index_ptr: null(),
    index_cnt: 0,
};
static mut T: f64 = 0.0;

#[no_mangle]
pub extern "C" fn init(index: u64) {
    unsafe {
        STATE = State::default();
        RENDER = match index {
            0 => Some(RenderData::Wave(<_>::new())),
            1 => Some(RenderData::DoubleJoint(<_>::new())),
            _ => None,
        };
    }
}

#[no_mangle]
pub extern "C" fn process(delta: f64) -> *const ExportState {
    unsafe {
        T += delta;
        if let Some(rp) = &mut *addr_of_mut!(RENDER) {
            rp.step(T as _, delta as _);
            rp.render(&mut *addr_of_mut!(STATE));
        };
        STATE_EXPORT = ExportState {
            vertex_ptr: STATE.vertex.as_ptr(),
            vertex_cnt: STATE.vertex.len(),
            normal_ptr: STATE.normal.as_ptr(),
            normal_cnt: STATE.normal.len(),
            tangent_ptr: STATE.tangent.as_ptr(),
            tangent_cnt: STATE.tangent.len(),
            uv_ptr: STATE.uv.as_ptr(),
            uv_cnt: STATE.uv.len(),
            color_ptr: STATE.color.as_ptr(),
            color_cnt: STATE.color.len(),
            index_ptr: STATE.index.as_ptr(),
            index_cnt: STATE.index.len(),
        };
        addr_of!(STATE_EXPORT)
    }
}

mod double_joint;
mod maze;
mod wave;

use std::cell::RefCell;
use std::fmt::{Arguments, Write as _};
use std::ptr::null;

use getrandom::Error as RandError;
use glam::f32::*;

#[link(wasm_import_module = "host")]
extern "C" {
    #[link_name = "log"]
    fn _log(p: *const u8, n: usize);
    #[link_name = "rand"]
    fn _rand(p: *mut u8, n: usize);
}

#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(dest: *mut u8, len: usize) -> Result<(), RandError> {
    // SAFETY: Wraps extern call
    unsafe { _rand(dest, len) }
    Ok(())
}

#[allow(dead_code)]
pub(crate) fn log(s: &str) {
    // SAFETY: Wraps extern call
    unsafe { _log(s.as_ptr(), s.len()) }
}

#[allow(dead_code)]
static mut TEMP_STR: RefCell<String> = RefCell::new(String::new());

#[allow(dead_code)]
pub(crate) fn print_log(args: Arguments) {
    // SAFETY: Wraps static mut
    let mut guard = unsafe { (*(&raw const TEMP_STR)).borrow_mut() };
    guard.clear();
    guard.write_fmt(args).unwrap();
    log(&guard);
}

#[macro_export]
macro_rules! log {
    ($($t:tt)*) => {
        if cfg!(debug_assertions) {
            $crate::print_log(format_args!($($t)*));
        }
    };
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Unknown,
}

trait Renderable {
    fn new() -> Self;
    fn render(&self, state: &mut State);
    fn step(&mut self, time: f32, delta: f32);
    fn click(&mut self, _origin: Vec3, _norm: Vec3, _button: MouseButton) {}
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

#[repr(C)]
pub struct ConfigItem {
    str_ptr: *const u8,
    str_len: usize,
}

impl ConfigItem {
    const fn from_str(s: &'static str) -> Self {
        Self {
            str_ptr: s.as_ptr(),
            str_len: s.len(),
        }
    }
}

#[repr(C)]
pub struct Config {
    cfg_ptr: *const ConfigItem,
    cfg_len: usize,
}

impl Config {
    const fn from_cfg(s: &'static [ConfigItem]) -> Self {
        Self {
            cfg_ptr: s.as_ptr(),
            cfg_len: s.len(),
        }
    }
}

enum RenderData {
    Wave(wave::Wave),
    DoubleJoint(double_joint::DoubleJoint),
    Maze(maze::Maze),
}

impl RenderData {
    fn config() -> *const Config {
        static mut CFG: Config = Config::from_cfg(&[
            ConfigItem::from_str("Wave"),
            ConfigItem::from_str("Double Joint"),
            ConfigItem::from_str("Maze"),
        ]);
        &raw const CFG
    }

    fn new(ix: u64) -> Option<Self> {
        match ix {
            0 => Some(Self::Wave(<_>::new())),
            1 => Some(Self::DoubleJoint(<_>::new())),
            2 => Some(Self::Maze(<_>::new())),
            _ => None,
        }
    }

    fn render(&self, state: &mut State) {
        match self {
            Self::Wave(v) => v.render(state),
            Self::DoubleJoint(v) => v.render(state),
            Self::Maze(v) => v.render(state),
        }
    }

    fn step(&mut self, time: f32, delta: f32) {
        match self {
            Self::Wave(v) => v.step(time, delta),
            Self::DoubleJoint(v) => v.step(time, delta),
            Self::Maze(v) => v.step(time, delta),
        }
    }

    fn click(&mut self, origin: Vec3, norm: Vec3, button: MouseButton) {
        match self {
            Self::Wave(v) => v.click(origin, norm, button),
            Self::DoubleJoint(v) => v.click(origin, norm, button),
            Self::Maze(v) => v.click(origin, norm, button),
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

#[unsafe(no_mangle)]
pub extern "C" fn config() -> *const Config {
    RenderData::config()
}

#[unsafe(no_mangle)]
pub extern "C" fn init(index: u64) {
    unsafe {
        STATE = State::default();
        RENDER = RenderData::new(index);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn process(delta: f64) -> *const ExportState {
    unsafe {
        T += delta;
        let state = &mut *(&raw mut STATE);
        if let Some(ref mut rp) = *(&raw mut RENDER) {
            rp.step(T as _, delta as _);
            rp.render(state);
        };
        STATE_EXPORT = ExportState {
            vertex_ptr: state.vertex.as_ptr(),
            vertex_cnt: state.vertex.len(),
            normal_ptr: state.normal.as_ptr(),
            normal_cnt: state.normal.len(),
            tangent_ptr: state.tangent.as_ptr(),
            tangent_cnt: state.tangent.len(),
            uv_ptr: state.uv.as_ptr(),
            uv_cnt: state.uv.len(),
            color_ptr: state.color.as_ptr(),
            color_cnt: state.color.len(),
            index_ptr: state.index.as_ptr(),
            index_cnt: state.index.len(),
        };
        &raw const STATE_EXPORT
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn click(ox: f32, oy: f32, oz: f32, nx: f32, ny: f32, nz: f32, button: u32) {
    let button = match button {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        _ => MouseButton::Unknown,
    };
    let origin = Vec3 {
        x: ox,
        y: oy,
        z: oz,
    };
    let norm = Vec3 {
        x: nx,
        y: ny,
        z: nz,
    };

    unsafe {
        if let Some(ref mut rp) = *(&raw mut RENDER) {
            rp.click(origin, norm, button);
        };
    }
}

mod game_of_life;
mod mandelbrot;

use std::cell::RefCell;
use std::fmt::{Arguments, Write as _};
use std::ptr::null;

#[link(wasm_import_module = "host")]
extern "C" {
    #[link_name = "log"]
    fn _log(p: *const u8, n: usize);
}

pub(crate) fn log(s: &str) {
    // SAFETY: Wraps extern call
    unsafe { _log(s.as_ptr(), s.len()) }
}

static mut TEMP_STR: RefCell<String> = RefCell::new(String::new());

pub(crate) fn print_log(args: Arguments) {
    // SAFETY: Wraps static mut
    let mut guard = unsafe { TEMP_STR.borrow_mut() };
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

trait Renderable {
    fn new() -> Self;
    fn render(&self, state: &mut State);
    fn step(&mut self, time: f32, delta: f32);
    fn click(&mut self, x: f32, y: f32, right_click: bool);
}

#[derive(Debug, Default, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[derive(Default)]
pub(crate) struct State {
    width: usize,
    height: usize,
    colors: Vec<Color>,
}

impl State {
    pub(crate) fn width(&self) -> usize {
        self.width
    }

    pub(crate) fn height(&self) -> usize {
        self.height
    }

    pub(crate) fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.colors.resize_with(width * height, Color::default);
    }

    pub fn colors(&self) -> &[Color] {
        &self.colors
    }

    pub fn colors_mut(&mut self) -> &mut [Color] {
        &mut self.colors
    }
}

#[repr(C)]
pub struct ExportState {
    pub width: usize,
    pub height: usize,
    pub colors_ptr: *const Color,
    pub colors_cnt: usize,
}

enum RenderData {
    Mandelbrot(mandelbrot::Mandelbrot),
    GameOfLife(game_of_life::GameOfLife),
}

impl RenderData {
    fn render(&self, state: &mut State) {
        match self {
            Self::Mandelbrot(v) => v.render(state),
            Self::GameOfLife(v) => v.render(state),
        }
    }

    fn step(&mut self, time: f32, delta: f32) {
        match self {
            Self::Mandelbrot(v) => v.step(time, delta),
            Self::GameOfLife(v) => v.step(time, delta),
        }
    }

    fn click(&mut self, x: f32, y: f32, right_click: bool) {
        match self {
            Self::Mandelbrot(v) => v.click(x, y, right_click),
            Self::GameOfLife(v) => v.click(x, y, right_click),
        }
    }
}

static mut RENDER: Option<RenderData> = None;
static mut STATE: State = State {
    width: 0,
    height: 0,
    colors: Vec::new(),
};
static mut STATE_EXPORT: ExportState = ExportState {
    width: 0,
    height: 0,
    colors_ptr: null(),
    colors_cnt: 0,
};
static mut T: f64 = 0.0;

#[no_mangle]
pub extern "C" fn init(index: u64) {
    unsafe {
        STATE = State::default();
        RENDER = match index {
            0 => Some(RenderData::Mandelbrot(<_>::new())),
            1 => Some(RenderData::GameOfLife(<_>::new())),
            _ => None,
        };
    }
}

#[no_mangle]
pub extern "C" fn process(delta: f64) -> *const ExportState {
    unsafe {
        T += delta;
        if let Some(rp) = &mut RENDER {
            rp.step(T as _, delta as _);
            rp.render(&mut STATE);
        };
        STATE_EXPORT = ExportState {
            width: STATE.width,
            height: STATE.height,
            colors_ptr: STATE.colors.as_ptr(),
            colors_cnt: STATE.colors.len(),
        };
        &STATE_EXPORT as _
    }
}

#[no_mangle]
pub extern "C" fn click(x: f64, y: f64, right_click: u32) {
    unsafe {
        if let Some(rp) = &mut RENDER {
            rp.click(x as _, y as _, right_click != 0);
        };
    }
}

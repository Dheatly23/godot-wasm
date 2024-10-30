mod game_of_life;
mod mandelbrot;
mod particles;

use std::cell::RefCell;
use std::fmt::{Arguments, Write as _};
use std::ptr::{addr_of, addr_of_mut, null};

use getrandom::{register_custom_getrandom, Error as RandError};

#[link(wasm_import_module = "host")]
extern "C" {
    #[link_name = "log"]
    fn _log(p: *const u8, n: usize);
    #[link_name = "rand"]
    fn _rand(p: *mut u8, n: usize);
}

fn custom_rand(buf: &mut [u8]) -> Result<(), RandError> {
    // SAFETY: Wraps extern call
    unsafe { _rand(buf.as_mut_ptr(), buf.len()) }
    Ok(())
}

register_custom_getrandom!(custom_rand);

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
    fn click(&mut self, x: f32, y: f32, button: MouseButton);
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

#[allow(dead_code)]
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

    pub(crate) fn colors(&self) -> &[Color] {
        &self.colors
    }

    pub(crate) fn colors_mut(&mut self) -> &mut [Color] {
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
    Mandelbrot(mandelbrot::Mandelbrot),
    GameOfLife(game_of_life::GameOfLife),
    Particles(particles::Particles),
}

impl RenderData {
    fn config() -> *const Config {
        static mut CFG: Config = Config::from_cfg(&[
            ConfigItem::from_str("Mandelbrot"),
            ConfigItem::from_str("Game of Life"),
            ConfigItem::from_str("Particle Sim"),
        ]);
        addr_of!(CFG)
    }

    fn new(ix: u64) -> Option<Self> {
        match ix {
            0 => Some(Self::Mandelbrot(<_>::new())),
            1 => Some(Self::GameOfLife(<_>::new())),
            2 => Some(Self::Particles(<_>::new())),
            _ => None,
        }
    }

    fn render(&self, state: &mut State) {
        match self {
            Self::Mandelbrot(v) => v.render(state),
            Self::GameOfLife(v) => v.render(state),
            Self::Particles(v) => v.render(state),
        }
    }

    fn step(&mut self, time: f32, delta: f32) {
        match self {
            Self::Mandelbrot(v) => v.step(time, delta),
            Self::GameOfLife(v) => v.step(time, delta),
            Self::Particles(v) => v.step(time, delta),
        }
    }

    fn click(&mut self, x: f32, y: f32, button: MouseButton) {
        match self {
            Self::Mandelbrot(v) => v.click(x, y, button),
            Self::GameOfLife(v) => v.click(x, y, button),
            Self::Particles(v) => v.click(x, y, button),
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
pub extern "C" fn config() -> *const Config {
    RenderData::config()
}

#[no_mangle]
pub extern "C" fn init(index: u64) {
    unsafe {
        STATE = State::default();
        RENDER = RenderData::new(index);
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
            width: STATE.width,
            height: STATE.height,
            colors_ptr: STATE.colors.as_ptr(),
            colors_cnt: STATE.colors.len(),
        };
        addr_of!(STATE_EXPORT)
    }
}

#[no_mangle]
pub extern "C" fn click(x: f64, y: f64, button: u32) {
    let button = match button {
        0 => MouseButton::Left,
        1 => MouseButton::Right,
        2 => MouseButton::Middle,
        _ => MouseButton::Unknown,
    };

    unsafe {
        if let Some(rp) = &mut *addr_of_mut!(RENDER) {
            rp.click(x as _, y as _, button);
        };
    }
}

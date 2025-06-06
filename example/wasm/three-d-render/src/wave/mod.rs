use colorgrad::Gradient;
use colorgrad::preset::turbo;

use crate::Color;

cfg_if::cfg_if! {
    if #[cfg(all(target_family = "wasm", target_feature = "simd128"))] {
        mod simd;
        pub use simd::Wave;
    } else {
        mod nosimd;
        pub use nosimd::Wave;
    }
}

const SIZE: usize = 64;
const WAVE_SCALE: usize = 8;
const TIME_SCALE: f32 = 1.0 / 1024.0;
const SPEED_SCALE: f32 = TIME_SCALE * 16.0;
const SPACE_SCALE: f32 = 5.0;
const MAX_REP: usize = 256;

fn map_color(mut v: f32) -> Color {
    v /= SPACE_SCALE;
    v = v * 0.5 + 0.5;
    let c = turbo().at(v);
    Color {
        r: c.r as _,
        g: c.g as _,
        b: c.b as _,
        a: 1.0,
    }
}

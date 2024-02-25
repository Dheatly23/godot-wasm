use colorous::TURBO;

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
const TIME_SCALE: f32 = 1.0 / 1024.0;
const SPEED_SCALE: f32 = TIME_SCALE * 16.0;
const SPACE_SCALE: f32 = 5.0;
const MAX_REP: usize = 256;

fn map_color(mut v: f32) -> Color {
    v = (v / SPACE_SCALE).clamp(-1.0, 1.0);
    v = v * 0.5 + 0.5;
    //v = (3.0 - v * v) * v * 0.25 + 0.5;
    let c = TURBO.eval_continuous(v as _);
    Color {
        r: (c.r as f32) / 255.0,
        g: (c.g as f32) / 255.0,
        b: (c.b as f32) / 255.0,
        a: 1.0,
    }
}

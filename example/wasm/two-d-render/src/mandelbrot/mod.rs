use colorgrad::preset::rd_yl_bu;
use colorgrad::Gradient;

use crate::Color;

cfg_if::cfg_if! {
    if #[cfg(all(target_family = "wasm", target_feature = "simd128"))] {
        mod simd;
        pub use simd::Mandelbrot;
    } else {
        mod nosimd;
        pub use nosimd::Mandelbrot;
    }
}

const SIZE: usize = 2048;
const STEPS: usize = 256;
const XMIN: f64 = -2.25;
const XMAX: f64 = 0.75;
const YMIN: f64 = -1.25;
const YMAX: f64 = 1.25;

fn map_color(v: f64) -> Color {
    let c = rd_yl_bu().reflect_at(v as _);
    Color {
        r: (c.r * 255.0) as _,
        g: (c.g * 255.0) as _,
        b: (c.b * 255.0) as _,
        a: 255,
    }
}

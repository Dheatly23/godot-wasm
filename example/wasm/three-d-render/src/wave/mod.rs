#[cfg(not(all(target_family = "wasm", target_feature = "simd128")))]
mod nosimd;
#[cfg(all(target_family = "wasm", target_feature = "simd128"))]
mod simd;

#[cfg(not(all(target_family = "wasm", target_feature = "simd128")))]
pub use nosimd::Wave;
#[cfg(all(target_family = "wasm", target_feature = "simd128"))]
pub use simd::Wave;

const SIZE: usize = 64;
const TIME_SCALE: f32 = 1.0 / 1024.0;
const SPEED_SCALE: f32 = TIME_SCALE * 16.0;
const MAX_REP: usize = 256;

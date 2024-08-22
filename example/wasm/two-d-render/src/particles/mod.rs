cfg_if::cfg_if! {
    if #[cfg(all(target_family = "wasm", target_feature = "simd128"))] {
        mod simd;
        pub use simd::Particles;
    } else {
        mod nosimd;
        pub use nosimd::Particles;
    }
}

const N_PARTICLES: usize = 512;
const PART_RADIUS: i32 = 1024;
const SIZE: usize = 512;
const TIME_SCALE: f32 = 1.0 / 1024.0;
const SPEED_SCALE_SHR: u32 = 8;
const MAX_REP: usize = 16;

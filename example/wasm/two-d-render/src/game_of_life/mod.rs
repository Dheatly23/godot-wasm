cfg_if::cfg_if! {
    if #[cfg(all(target_family = "wasm", target_feature = "simd128"))] {
        mod simd;
        pub use simd::GameOfLife;
    } else {
        mod nosimd;
        pub use nosimd::GameOfLife;
    }
}

const SIZE: usize = 512;

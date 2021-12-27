1. build `libfdt-binding.a` with cmake
2. copy `libfdt-binding.a` to Rust link search path
3. add `cargo:rustc-link-lib=static=fdt-binding` to `build.rs`
4. add `libfdt-binding` crate to `Cargo.toml`
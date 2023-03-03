use std::env::var;
use cmake::Config;

fn main() {
    // compile libfdt-bingding with cmake
    let dst = Config::new("libfdt-binding")
        .profile("release")
        .build_target("all")
        .build()
        .join("build");
    println!("cargo:rustc-link-search=native={}", dst.display());
    println!("cargo:rustc-link-lib=static=fdt-binding");

    // set the linker script
    let arch = var("CARGO_CFG_TARGET_ARCH").unwrap();
    let platform = if cfg!(feature = "tx2") {
        "tx2"
    } else if cfg!(feature = "pi4") {
        "pi4"
    } else if cfg!(feature = "qemu") {
        "qemu"
    } else {
        panic!("Unsupported platform!");
    };
    let linker_ld = format!("{}-{}.ld", arch, platform);
    println!("cargo:rustc-link-arg=-Tsrc/linkers/{}", linker_ld);

    // compile the startup file into a static library
    cc::Build::new()
        .compiler("aarch64-none-elf-gcc")
        .file(format!("src/arch/{}/start.S", arch))
        .define(&format!("PLATFORM_{}", platform.to_uppercase()), None)
        .compile("start");

    // set envs
    let build_time = chrono::offset::Local::now().format("%Y-%m-%d %H:%M:%S %Z");
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    println!("cargo:rustc-env=PLATFORM={}", platform.to_uppercase());
}

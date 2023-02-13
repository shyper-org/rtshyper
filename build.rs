use std::env::var;

fn main() {
    println!("cargo:rustc-link-search=native={}/lib", var("PWD").unwrap());
    println!("cargo:rustc-link-lib=static=fdt-binding");

    let build_time = chrono::offset::Local::now().format("%Y-%m-%d %H:%M:%S %Z");
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);

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
    println!("cargo:rustc-env=PLATFORM={}", platform.to_uppercase());
    let linker_ld = format!("{}-{}.ld", arch, platform);
    println!("cargo:rustc-link-arg=-Tsrc/linkers/{}", linker_ld);
}

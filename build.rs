use std::env::var;
use std::process::Command;

fn main() {
    println!("cargo:rustc-link-search=native={}/lib", var("PWD").unwrap());
    println!("cargo:rustc-link-lib=static=fdt-binding");
    // note: add error checking yourself.
    let output = Command::new("date").arg("+\"%Y-%m-%d %H:%M:%S %Z\"").output().unwrap();
    let build_time = String::from_utf8(output.stdout).unwrap();
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
    let appendix = if cfg!(feature = "update") { "-update" } else { "" };
    let linker_ld = String::from(format!("{}-{}{}.ld", arch, platform, appendix));
    println!("cargo:rustc-link-arg=-Tsrc/linkers/{}", linker_ld);
}

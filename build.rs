use std::env::var;
use std::process::Command;

fn main() {
    println!("cargo:rustc-link-search=native={}/lib", var("PWD").unwrap());
    println!("cargo:rustc-link-lib=static=fdt-binding");
    // note: add error checking yourself.
    let output = Command::new("date").arg("+\"%Y-%m-%d %H:%M:%S %Z\"").output().unwrap();
    let build_time = String::from_utf8(output.stdout).unwrap();
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
}

use std::env::var;

fn main() {
    println!("cargo:rustc-link-search=native={}/lib", var("PWD").unwrap());
    println!("cargo:rustc-link-lib=static=tegra");
}

use std::env::var;

fn main() {
    // compile libfdt-bingding
    let c_compiler = "aarch64-none-elf-gcc";
    let files = std::fs::read_dir("deps/libfdt")
        .unwrap()
        .into_iter()
        .chain(std::fs::read_dir("libfdt-binding").unwrap().into_iter())
        .filter_map(|f| {
            let f = f.as_ref().unwrap();
            if f.file_type().unwrap().is_file() && matches!(f.path().extension(), Some(ext) if ext == "c") {
                Some(f.path())
            } else {
                None
            }
        });
    cc::Build::new()
        .compiler(c_compiler)
        .includes(["libfdt-binding", "deps/libfdt"])
        .files(files)
        .flag("-w")
        .compile("fdt-binding");

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
    println!("cargo:rustc-link-arg=-Tlinkers/{}", linker_ld);

    // compile the startup file into a static library
    let start_file = format!("src/arch/{}/start.S", arch);
    println!("cargo:rerun-if-changed=src/");
    cc::Build::new()
        .compiler(c_compiler)
        .file(start_file)
        .define(&format!("PLATFORM_{}", platform.to_uppercase()), None)
        .compile("start");

    // set envs
    let build_time = chrono::offset::Local::now().format("%Y-%m-%d %H:%M:%S %Z");
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    println!("cargo:rustc-env=PLATFORM={}", platform.to_uppercase());
}

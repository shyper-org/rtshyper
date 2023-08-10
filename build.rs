#![feature(absolute_path)]
use std::env::var;

struct ConfigPlatform {
    platform: &'static str,
    vm0_image_path: &'static str,
    text_start: usize,
}

const fn get_config() -> ConfigPlatform {
    if cfg!(feature = "tx2") {
        ConfigPlatform {
            platform: "tx2",
            vm0_image_path: "image/L4T",
            text_start: 0x83000000,
        }
    } else if cfg!(feature = "pi4") {
        ConfigPlatform {
            platform: "pi4",
            vm0_image_path: "image/Image_pi4_5.4.83_tlb",
            text_start: 0xf0080000,
        }
    } else if cfg!(feature = "qemu") {
        ConfigPlatform {
            platform: "qemu",
            vm0_image_path: "image/Image_vanilla",
            text_start: 0x40080000,
        }
    } else {
        panic!("Unsupported platform!");
    }
}

fn main() {
    println!("cargo:rerun-if-changed=src/");

    // compile libfdt-bingding
    let c_compiler = "aarch64-none-elf-gcc";
    let fdt_dirs = ["libfdt-binding", "deps/libfdt"];
    let c_files = fdt_dirs.iter().flat_map(|path| {
        std::fs::read_dir(path).unwrap().filter_map(|f| {
            let f = f.unwrap();
            if f.file_type().unwrap().is_file() && matches!(f.path().extension(), Some(ext) if ext == "c") {
                Some(f.path())
            } else {
                None
            }
        })
    });
    cc::Build::new()
        .compiler(c_compiler)
        .includes(fdt_dirs)
        .files(c_files)
        .flag("-w")
        .compile("fdt-binding");

    // set the linker script
    let arch = var("CARGO_CFG_TARGET_ARCH").unwrap();
    println!("cargo:rustc-link-arg=-Tlinkers/{arch}.ld");
    let config = get_config();
    println!("cargo:rustc-link-arg=--defsym=TEXT_START={}", config.text_start);

    // set envs
    let build_time = chrono::offset::Local::now().format("%Y-%m-%d %H:%M:%S %Z");
    println!("cargo:rustc-env=BUILD_TIME={}", build_time);
    println!(
        "cargo:rustc-env=VM0_IMAGE_PATH={}/{}",
        env!("CARGO_MANIFEST_DIR"),
        config.vm0_image_path
    );
    println!("cargo:rustc-env=PLATFORM={}", config.platform.to_uppercase());
}

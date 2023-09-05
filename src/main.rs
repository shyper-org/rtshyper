#![cfg_attr(not(test), no_std)]
#![cfg_attr(not(test), no_main)]
#![feature(alloc_error_handler)]
#![feature(const_btree_new)]
#![feature(drain_filter)]
#![feature(inline_const)]
#![feature(const_refs_to_cell)]
#![feature(const_cmp)]
#![feature(binary_heap_retain)]
#![feature(naked_functions)]
#![feature(asm_sym)]
#![feature(asm_const)]
#![allow(unused_doc_comments)]

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate log;
#[macro_use]
extern crate static_assertions;
#[macro_use]
extern crate memoffset;
#[macro_use]
extern crate derive_more;

#[macro_use]
mod macros;

mod arch;
mod banner;
mod board;
mod config;
mod device;
mod driver;
mod dtb;
mod feature;
mod kernel;
mod mm;
mod panic;
#[allow(dead_code)]
mod util;
mod vmm;

mod built_info {
    include!(concat!(env!("OUT_DIR"), "/built.rs"));
}

fn print_built_info() {
    println!(
        "Welcome to {} {} {} Hypervisor!",
        env!("PLATFORM"),
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION")
    );
    println!(
        "Built At {build_time} by {hostname} {rustc_version} with features {features:?}",
        build_time = env!("BUILD_TIME"),
        hostname = env!("HOSTNAME"),
        rustc_version = built_info::RUSTC_VERSION,
        features = built_info::FEATURES_LOWERCASE_STR
    );
}

pub fn init(cpu_id: usize, dtb: *mut core::ffi::c_void) -> ! {
    if cpu_id == 0 {
        driver::init();
        banner::init();
        print_built_info();
        util::logger::logger_init().unwrap();
        mm::init(); // including heap and hypervisor VA space

        dtb::init_vm0_dtb(dtb);
        kernel::physical_mem_init();
        #[cfg(feature = "iommu")]
        kernel::iommu_init();
    }
    kernel::cpu_init();
    kernel::timer_init();
    util::barrier();
    kernel::hypervisor_self_coloring();
    if cpu_id == 0 {
        kernel::subinit();
        vmm::vm_init();
        info!(
            "{} Hypervisor init ok\n\nStart booting Monitor VM ...",
            env!("CARGO_PKG_NAME")
        );
        vmm::vmm_boot_vm(0);
    }

    use kernel::current_cpu;
    current_cpu().vcpu_array.resched();
    extern "C" {
        fn context_vm_entry(ctx: usize) -> !;
    }
    unsafe {
        context_vm_entry(current_cpu().current_ctx() as usize);
    }
}

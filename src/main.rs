#![no_std]
#![no_main]
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

use device::mediated_dev_init;
use kernel::{cpu_init, physical_mem_init, timer_init, hvc_init, iommu_init, current_cpu};
use vmm::{vm_init, vmm_boot_vm};

#[macro_use]
mod macros;

mod arch;
mod banner;
mod board;
mod config;
mod device;
mod driver;
mod dtb;
mod kernel;
mod mm;
mod panic;
mod util;
mod vmm;

#[no_mangle]
pub fn init(cpu_id: usize, dtb: *mut core::ffi::c_void) -> ! {
    if cpu_id == 0 {
        driver::init();
        banner::init();
        util::logger::logger_init().unwrap();
        info!("Welcome to {} {} Hypervisor!", env!("PLATFORM"), env!("CARGO_PKG_NAME"));
        info!("Built At {}", env!("BUILD_TIME"));

        mm::init(); // including heap and hypervisor VA space
        physical_mem_init();
        dtb::init_vm0_dtb(dtb);
        hvc_init();
        iommu_init();
        mediated_dev_init();
    }
    cpu_init();
    timer_init();
    util::barrier();
    kernel::hypervisor_self_coloring();
    if cpu_id == 0 {
        vm_init();
        info!(
            "{} Hypervisor init ok\n\nStart booting Monitor VM ...",
            env!("CARGO_PKG_NAME")
        );
        vmm_boot_vm(0);
    }

    current_cpu().vcpu_array.resched();
    extern "C" {
        fn context_vm_entry(ctx: usize) -> !;
    }
    unsafe {
        context_vm_entry(current_cpu().current_ctx().unwrap());
    }
}

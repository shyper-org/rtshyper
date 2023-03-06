#![no_std]
#![no_main]
#![feature(default_alloc_error_handler)]
#![feature(alloc_error_handler)]
#![feature(const_btree_new)]
#![feature(drain_filter)]
#![feature(inline_const)]
#![allow(unused_doc_comments)]
#![allow(special_module_name)]

#[macro_use]
extern crate alloc;
extern crate fdt;
#[macro_use]
// extern crate lazy_static;
extern crate log;
#[macro_use]
extern crate static_assertions;

use device::{init_vm0_dtb, mediated_dev_init};
use kernel::{cpu_init, interrupt_init, mem_init, timer_init};
use mm::heap_init;
use vmm::{vm_init, vmm_boot_vm};

use crate::kernel::{cpu_sched_init, hvc_init, iommu_init};

#[macro_use]
mod macros;

#[allow(dead_code)]
mod arch;
mod board;
mod config;
#[allow(dead_code)]
mod device;
#[allow(dead_code)]
mod driver;
#[allow(dead_code)]
mod kernel;
#[allow(dead_code)]
mod lib;
mod mm;
mod panic;
mod vmm;

#[no_mangle]
pub fn init(cpu_id: usize, dtb: *mut fdt::myctypes::c_void) -> ! {
    if cpu_id == 0 {
        println!("Welcome to {} {} Hypervisor!", env!("PLATFORM"), env!("CARGO_PKG_NAME"));
        println!("Built At {}", env!("BUILD_TIME"));

        #[cfg(feature = "pi4")]
        {
            crate::driver::gpio_select_function(0, 4);
            crate::driver::gpio_select_function(1, 4);
        }

        heap_init();
        let _ = kernel::logger_init();
        mem_init();
        init_vm0_dtb(dtb);
        hvc_init();
        iommu_init();
    }
    cpu_init();
    interrupt_init();
    timer_init();
    cpu_sched_init();
    if cpu_id == 0 {
        mediated_dev_init();
    }
    crate::lib::barrier();
    if cpu_id != 0 {
        crate::kernel::cpu_idle();
    }
    vm_init();
    println!(
        "{} Hypervisor init ok\n\nStart booting Monitor VM ...",
        env!("CARGO_PKG_NAME")
    );
    vmm_boot_vm(0);

    loop {
        core::hint::spin_loop();
    }
}

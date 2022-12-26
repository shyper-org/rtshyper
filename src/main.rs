#![no_std]
#![no_main]
#![feature(core_intrinsics)]
#![feature(default_alloc_error_handler)]
#![feature(alloc_error_handler)]
#![feature(const_btree_new)]
#![feature(drain_filter)]
#![allow(unused_doc_comments)]
#![allow(special_module_name)]

#[macro_use]
extern crate alloc;
extern crate fdt;
#[macro_use]
// extern crate lazy_static;
extern crate log;

// extern crate rlibc;

use device::{init_vm0_dtb, mediated_dev_init};
use kernel::{cpu_init, interrupt_init, mem_init, timer_init};
use mm::heap_init;
use vmm::{vm_init, vmm_boot_vm};

use crate::kernel::{cpu_sched_init, hvc_init, iommu_init};

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::lib::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[allow(dead_code)]
mod arch;
#[allow(dead_code)]
mod board;
#[allow(dead_code)]
mod config;
#[allow(dead_code)]
mod device;
#[allow(dead_code)]
mod driver;
#[allow(dead_code)]
mod kernel;
#[allow(dead_code)]
mod lib;
#[allow(dead_code)]
mod mm;
#[allow(dead_code)]
mod panic;
#[allow(dead_code)]
mod vmm;

// use lib::{BitAlloc, BitAlloc256};

pub static SYSTEM_FDT: spin::Once<alloc::vec::Vec<u8>> = spin::Once::new();

#[no_mangle]
pub fn init(cpu_id: usize, dtb: *mut fdt::myctypes::c_void) {
    // const UART0: *mut u8 = 0x0900_0000 as *mut u8;
    // let out_str = b"AArch64 Bare Metal";
    // for byte in out_str {
    //     crate::driver::uart::putc(*byte);
    // }
    // tegra_emmc_blk_read(0, 0, 0 as *mut _);
    if cpu_id == 0 {
        #[cfg(feature = "tx2")]
        println!("Welcome to TX2 Rust-Shyper Hypervisor!");
        #[cfg(feature = "qemu")]
        println!("Welcome to Qemu Rust-Shyper Hypervisor!");
        #[cfg(feature = "pi4")]
        println!("Welcome to PI4 Rust-Shyper Hypervisor!");
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
    println!("Sybilla Hypervisor init ok\n\nStart booting Monitor VM ...");
    vmm_boot_vm(0);

    loop {}
}

#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(core_intrinsics)]
#![feature(default_alloc_error_handler)]
#![feature(alloc_error_handler)]
#![feature(llvm_asm)]
#![feature(const_fn)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate log;
extern crate rlibc;

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::lib::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

mod arch;
mod board;
mod config;
mod device;
mod driver;
mod kernel;
mod lib;
mod mm;
mod panic;
mod vmm;

use board::platform_blk_init;
use kernel::{cpu_init, interrupt_init, mem_init, timer_init};
use lib::fs_init;
use mm::heap_init;
use vmm::{vmm_boot, vmm_init};
// use lib::{BitAlloc, BitAlloc256};

#[no_mangle]
pub unsafe fn init(cpu_id: usize) {
    // println!("core id {}", cpu_id);
    // const UART0: *mut u8 = 0x0900_0000 as *mut u8;
    // let out_str = b"AArch64 Bare Metal";
    // for byte in out_str {
    //     crate::driver::uart::putc(*byte);
    // }
    // tegra_emmc_blk_read(0, 0, 0 as *mut _);
    if cpu_id == 0 {
        #[cfg(feature = "tx2")]
        println!("Welcome to TX2 Sybilla Hypervisor!");
        #[cfg(feature = "qemu")]
        println!("Welcome to Qemu Sybilla Hypervisor!");
        heap_init();
        mem_init();
        // kernel::logger_init();
    }
    cpu_init();
    interrupt_init();
    timer_init();

    if cpu_id == 0 {
        platform_blk_init();
        fs_init();
    }

    vmm_init();
    vmm_boot();

    loop {}
}

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
extern crate alloc;
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
use spin::Mutex;
use vmm::vmm_init;
// use lib::{BitAlloc, BitAlloc256};

#[no_mangle]
pub extern "C" fn init(cpu_id: usize) {
    // println!("core id {}", core_id);
    // const UART0: *mut u8 = 0x0900_0000 as *mut u8;
    // let out_str = b"AArch64 Bare Metal";
    // for byte in out_str {
    //     crate::driver::uart::putc(*byte);
    // }
    if cpu_id == 0 {
        println!("Welcome to Sybilla Hypervisor!");
        heap_init();
        mem_init();
    }
    cpu_init();
    interrupt_init();
    timer_init();

    if cpu_id == 0 {
        platform_blk_init();
        // TODO: check fs
        fs_init();
    }

    vmm_init();

    loop {}
}

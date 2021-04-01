#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(core_intrinsics)]
#![feature(default_alloc_error_handler)]
#![feature(alloc_error_handler)]
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
mod driver;
mod kernel;
mod lib;
mod mm;
mod panic;

use kernel::mem_init;
use mm::heap;
// use lib::{BitAlloc, BitAlloc256};

// #[no_mangle]
// static mut cpu: u32 = 1;

#[no_mangle]
pub extern "C" fn init() {
    // const UART0: *mut u8 = 0x0900_0000 as *mut u8;
    // let out_str = b"AArch64 Bare Metal";
    // for byte in out_str {
    //     crate::driver::uart::putc(*byte);
    // }
    // panic!("lalal");
    println!("Welcome to Sybilla Hypervisor!");
    heap::init();
    mem_init();

    loop {}
}

#[no_mangle]
#[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self() {}

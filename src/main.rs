#![no_std]
#![no_main]
#![feature(global_asm)]

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
mod panic;

use kernel::mem_init;

#[no_mangle]
static mut cpu: u32 = 1;
#[no_mangle]
static mut vectors: u32 = 1;

#[no_mangle]
pub extern "C" fn init() {
    // const UART0: *mut u8 = 0x0900_0000 as *mut u8;
    // let out_str = b"AArch64 Bare Metal";
    // for byte in out_str {
    //     crate::driver::uart::putc(*byte);
    // }
    println!("Welcome to Sybilla Hypervisor!");
    mem_init();

    loop {}
}

#[no_mangle]
#[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self() {}

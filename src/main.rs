#![no_std]
#![no_main]
#![feature(global_asm)]

extern crate rlibc;

mod arch;
mod driver;
mod lib;
mod panic;

use arch::PAGE_SIZE;
// use core::ptr;
// use spin::Once;

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

    loop {}
}

#[no_mangle]
#[link_section = ".text.boot"]
pub extern "C" fn cpu_map_self() {}

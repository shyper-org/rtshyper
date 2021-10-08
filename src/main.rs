#![no_std]
#![no_main]
#![feature(global_asm)]
#![feature(core_intrinsics)]
#![feature(default_alloc_error_handler)]
#![feature(alloc_error_handler)]
#![feature(asm)]
#![feature(const_fn_trait_bound)]

#[macro_use]
extern crate lazy_static;

#[macro_use]
extern crate alloc;
#[macro_use]
extern crate log;
// extern crate rlibc;

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
use mm::heap_init;
use vmm::{vmm_boot, vmm_init};
// use lib::{BitAlloc, BitAlloc256};

extern crate fdt;

pub static SYSTEM_FDT: spin::Once<alloc::vec::Vec<u8>> = spin::Once::new();

#[no_mangle]
pub unsafe fn init(cpu_id: usize, dtb: *mut fdt::myctypes::c_void) {
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
        println!(
            "Welcome to Qemu Sybilla 
        Hypervisor!"
        );
        heap_init();
        mem_init();
        // kernel::logger_init();

        unsafe {
            use fdt::*;
            println!("fdt orignal size {}", fdt_size(dtb));
            fdt_pack(dtb);
            fdt_enlarge(dtb);
            let r = fdt_del_mem_rsv(dtb, 0);
            assert_eq!(r, 0);
            // fdt_add_mem_rsv(fdt, 0x80000000, 0x10000000);
            let r = fdt_clear_initrd(dtb);
            let r = fdt_remove_node(dtb, "/cpus/cpu-map/cluster0/core0\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_remove_node(dtb, "/cpus/cpu-map/cluster0/core1\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/cpus/cpu@0\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/cpus/cpu@1\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/serial@c280000\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/serial@3110000\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/serial@3130000\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/combined-uart\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/trusty\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/host1x/nvdisplay@15210000\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/reserved-memory/ramoops_carveout\0".as_ptr());
            assert_eq!(r, 0);
            let r = fdt_disable_node(dtb, "/watchdog@30c0000\0".as_ptr());
            assert_eq!(r, 0);
            let len = fdt_size(dtb);
            println!("fdt after patched size {}", len);
            let slice = core::slice::from_raw_parts(dtb as *const u8, len as usize);

            SYSTEM_FDT.call_once(|| slice.to_vec());
        }
    }
    cpu_init();
    interrupt_init();
    timer_init();

    vmm_init();
    vmm_boot();

    loop {}
}

use core::ptr;

use crate::board::{Platform, PlatOperation};

pub fn putc(byte: u8) {
    const UART_BASE: usize = Platform::HYPERVISOR_UART_BASE;
    #[cfg(feature = "qemu")]
    unsafe {
        ptr::write_volatile(UART_BASE as *mut u8, byte);
    }
    // ns16550
    #[cfg(feature = "tx2")]
    unsafe {
        if byte == b'\n' {
            putc(b'\r');
        }
        while ptr::read_volatile((UART_BASE + 20) as *const u8) & 0x20 == 0 {}
        ptr::write_volatile(UART_BASE as *mut u8, byte);
        // while ptr::read_volatile((UART_1_ADDR + 20) as *const u8) & 0x20 == 0 {}
        // ptr::write_volatile(UART_1_ADDR as *mut u8, byte);
    }
    // pl011
    #[cfg(feature = "pi4")]
    unsafe {
        if byte == b'\n' {
            putc(b'\r');
        }
        while (ptr::read_volatile((UART_BASE + 24) as *const u32) & (1 << 5)) != 0 {}
        ptr::write_volatile(UART_BASE as *mut u32, byte as u32);
    }
}

use core::ptr;

use crate::board::{Platform, PlatOperation};

pub fn putc(byte: u8) {
    #[cfg(feature = "qemu")]
    unsafe {
        ptr::write_volatile(Platform::UART_0_ADDR as *mut u8, byte);
    }
    // ns16550
    #[cfg(feature = "tx2")]
    unsafe {
        use crate::arch::DEVICE_BASE;
        if byte == b'\n' {
            putc(b'\r');
        }
        while ptr::read_volatile((Platform::UART_1_ADDR + DEVICE_BASE + 20) as *const u8) & 0x20 == 0 {}
        ptr::write_volatile((Platform::UART_1_ADDR + DEVICE_BASE) as *mut u8, byte);
        // while ptr::read_volatile((UART_1_ADDR + 20) as *const u8) & 0x20 == 0 {}
        // ptr::write_volatile(UART_1_ADDR as *mut u8, byte);
    }
    // pl011
    #[cfg(feature = "pi4")]
    unsafe {
        if byte == b'\n' {
            putc(b'\r');
        }
        while (ptr::read_volatile((Platform::UART_0_ADDR as usize + 24) as *const u32) & (1 << 5)) != 0 {}
        ptr::write_volatile(Platform::UART_0_ADDR as *mut u32, byte as u32);
    }
}

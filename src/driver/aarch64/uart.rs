use core::ptr;

pub fn putc(byte: u8) {
    use crate::board::UART_1_ADDR;
    #[cfg(feature = "qemu")]
        unsafe {
        ptr::write_volatile(UART_1_ADDR as *mut u8, byte);
    }
    #[cfg(feature = "tx2")]
        unsafe {
        if byte == '\n' as u8 {
            putc('\r' as u8);
        }
        while ptr::read_volatile((UART_1_ADDR + 0x8_0000_0000 + 20) as *const u8) & 0x20 == 0 {}
        ptr::write_volatile((UART_1_ADDR + 0x8_0000_0000) as *mut u8, byte);
        // while ptr::read_volatile((UART_1_ADDR + 20) as *const u8) & 0x20 == 0 {}
        // ptr::write_volatile(UART_1_ADDR as *mut u8, byte);
    }
}

use core::ptr;

const UART0: *mut u8 = 0x0900_0000 as *mut u8;

pub fn putc(byte: u8) {
    unsafe {
        ptr::write_volatile(UART0, byte);
    }
}

#[allow(dead_code)]
pub fn writeb(value: u8, addr: usize) {
    unsafe {
        *(addr as *mut u8) = value;
    }
}

#[allow(dead_code)]
pub fn writew(value: u16, addr: usize) {
    unsafe {
        *(addr as *mut u16) = value;
    }
}

#[allow(dead_code)]
pub fn writel(value: u32, addr: usize) {
    unsafe {
        *(addr as *mut u32) = value;
    }
}

#[allow(dead_code)]
pub fn writeq(value: u64, addr: usize) {
    unsafe {
        *(addr as *mut u64) = value;
    }
}

#[allow(dead_code)]
pub fn readb(addr: usize) -> u8 {
    unsafe { *(addr as *const u8) }
}

#[allow(dead_code)]
pub fn readw(addr: usize) -> u16 {
    unsafe { *(addr as *const u16) }
}

#[allow(dead_code)]
pub fn readl(addr: usize) -> u32 {
    unsafe { *(addr as *const u32) }
}

#[allow(dead_code)]
pub fn readq(addr: usize) -> u64 {
    unsafe { *(addr as *const u64) }
}

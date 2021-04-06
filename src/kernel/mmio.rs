pub fn writeb(value: u8, addr: usize) {
    unsafe {
        *(addr as *mut u8) = value;
    }
}

pub fn writew(value: u16, addr: usize) {
    unsafe {
        *(addr as *mut u16) = value;
    }
}

pub fn writel(value: u32, addr: usize) {
    unsafe {
        *(addr as *mut u32) = value;
    }
}

pub fn writeq(value: u64, addr: usize) {
    unsafe {
        *(addr as *mut u64) = value;
    }
}

pub fn readb(addr: usize) -> u8 {
    unsafe { *(addr as *const u8) }
}

pub fn readw(addr: usize) -> u16 {
    unsafe { *(addr as *const u16) }
}

pub fn readl(addr: usize) -> u32 {
    unsafe { *(addr as *const u32) }
}

pub fn readq(addr: usize) -> u64 {
    unsafe { *(addr as *const u64) }
}

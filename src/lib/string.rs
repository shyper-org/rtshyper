// use core::arch::global_asm;

// global_asm!(include_str!("../arch/aarch64/memset.S"));
// global_asm!(include_str!("../arch/aarch64/memcpy.S"));
extern "C" {
    pub fn memset(s: *mut u8, c: i32, n: usize) -> *mut u8;
    pub fn memcpy(dest: *const u8, src: *const u8, n: usize) -> *mut u8;
}

pub fn memset_safe(s: *mut u8, c: i32, n: usize) -> *mut u8 {
    if (s as usize) < 0x1000 {
        panic!("illegal addr for memset s {:x}", s as usize);
    }
    unsafe { memset(s, c, n) }
    // unsafe {
    //     core::ptr::write_bytes(s, c as u8, n);
    // }
    // s
}

pub fn memcpy_safe(dest: *const u8, src: *const u8, n: usize) -> *mut u8 {
    if (dest as usize) < 0x1000 || (src as usize) < 0x1000 {
        panic!("illegal addr for memcpy s1 {:x} s2 {:x}", dest as usize, src as usize);
    }
    unsafe { memcpy(dest, src, n) }
    // unsafe {
    //     core::ptr::copy_nonoverlapping(src, dest as *mut _, n);
    // }
    // dest as *mut _
}

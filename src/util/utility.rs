use core::ptr;

use crate::arch::PAGE_SIZE;

#[inline(always)]
pub fn round_up(value: usize, to: usize) -> usize {
    ((value + to - 1) / to) * to
}

#[inline(always)]
pub fn round_down(value: usize, to: usize) -> usize {
    value & !(to - 1)
}

#[inline(always)]
pub fn byte2page(byte: usize) -> usize {
    round_up(byte, PAGE_SIZE) / PAGE_SIZE
}

#[inline(always)]
pub fn bit_extract(bits: usize, off: usize, len: usize) -> usize {
    (bits >> off) & ((1 << len) - 1)
}

#[inline(always)]
pub fn bit_get(bits: usize, off: usize) -> usize {
    (bits >> off) & 1
}

#[inline(always)]
pub fn bit_set(bits: usize, off: usize) -> usize {
    bits | (1 << off)
}

// change find nth
pub fn bitmap_find_nth(bitmap: usize, start: usize, size: usize, nth: usize, set: bool) -> Option<usize> {
    if size + start > 64 {
        println!("bitmap_find_nth: bitmap size is too large");
        return None;
    }
    let mut count = 0;
    let bit = usize::from(set);
    let end = start + size;

    for i in start..end {
        if bit_extract(bitmap, i, 1) == bit {
            count += 1;
            if count == nth {
                return Some(i);
            }
        }
    }

    None
}

pub fn ptr_read_write(addr: usize, width: usize, val: usize, read: bool) -> usize {
    if read {
        if width == 1 {
            unsafe { ptr::read(addr as *const u8) as usize }
        } else if width == 2 {
            unsafe { ptr::read(addr as *const u16) as usize }
        } else if width == 4 {
            unsafe { ptr::read(addr as *const u32) as usize }
        } else if width == 8 {
            unsafe { ptr::read(addr as *const u64) as usize }
        } else {
            panic!("ptr_read_write: illegal read len {}", width);
        }
    } else {
        if width == 1 {
            unsafe {
                ptr::write(addr as *mut u8, val as u8);
            }
        } else if width == 2 {
            unsafe {
                ptr::write(addr as *mut u16, val as u16);
            }
        } else if width == 4 {
            unsafe {
                ptr::write(addr as *mut u32, val as u32);
            }
        } else if width == 8 {
            unsafe {
                ptr::write(addr as *mut u64, val as u64);
            }
        } else {
            panic!("ptr_read_write: illegal write len {}", width);
        }
        0
    }
}

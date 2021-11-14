use crate::arch::PAGE_SIZE;
use spin::Mutex;

pub static TRACE: Mutex<bool> = Mutex::new(true);

pub fn set_trace(value: bool) {
    let mut trace = TRACE.lock();
    *trace = value;
}

pub fn trace() -> bool {
    let trace = TRACE.lock();
    *trace
}

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
pub fn range_in_range(base1: usize, size1: usize, base2: usize, size2: usize) -> bool {
    (base1 >= base2) && ((base1 + size1) <= (base2 + size2))
}

#[inline(always)]
pub fn in_range(addr: usize, base: usize, size: usize) -> bool {
    range_in_range(addr, 0, base, size)
}

#[inline(always)]
pub fn bit_extract(bits: usize, off: usize, len: usize) -> usize {
    (bits >> off) & ((1 << len) - 1)
}

#[inline(always)]
pub fn bit_get(bits: usize, off: usize) -> usize {
    bits & (1 << off)
}

#[inline(always)]
pub fn bit_set(bits: usize, off: usize) -> usize {
    bits | (1 << off)
}

#[inline(always)]
pub fn bit_num(bits: usize, size: usize) -> usize {
    let mut num = 0;
    for i in 0..size {
        if bits & 1 << i != 0 {
            num += 1;
        }
    }
    num
}

// change find nth
pub fn bitmap_find_nth(
    bitmap: usize,
    start: usize,
    size: usize,
    nth: usize,
    set: bool,
) -> Option<usize> {
    if size + start > 64 {
        println!("bitmap_find_nth: bitmap size is too large");
        return None;
    }
    let mut count = 0;
    let bit = if set { 1 } else { 0 };
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

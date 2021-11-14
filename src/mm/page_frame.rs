use crate::lib::{memset_safe, trace};

use crate::arch::PAGE_SIZE;

#[derive(Clone, Debug)]
pub struct PageFrame {
    pa: usize,
}

impl PageFrame {
    pub fn new(pa: usize) -> Self {
        assert_eq!(pa % PAGE_SIZE, 0);
        PageFrame { pa }
    }

    pub fn pa(&self) -> usize {
        self.pa
    }

    #[allow(dead_code)]
    pub fn zero(&self) {
        memset_safe(self.pa as *mut u8, 0, PAGE_SIZE);
    }

    pub fn as_slice<T>(&self) -> &'static [T] {
        if trace() && self.pa() < 0x1000 {
            panic!("illegal addr {:x}", self.pa());
        }
        unsafe {
            core::slice::from_raw_parts(self.pa as *const T, PAGE_SIZE / core::mem::size_of::<T>())
        }
    }
    #[allow(dead_code)]
    pub fn as_mut_slice<T>(&self) -> &'static mut [T] {
        if trace() && self.pa() < 0x1000 {
            panic!("illegal addr {:x}", self.pa());
        }
        unsafe {
            core::slice::from_raw_parts_mut(self.pa as *mut T, PAGE_SIZE / core::mem::size_of::<T>())
        }
    }
}

use crate::kernel::mem_pages_free;

impl Drop for PageFrame {
    fn drop(&mut self) {
        mem_pages_free(self.pa, 1);
    }
}

use crate::arch::PAGE_SIZE;
use crate::kernel::{mem_heap_free, mem_heap_alloc, AllocError};
use crate::lib::{memset_safe, trace};

#[derive(Debug)]
pub struct PageFrame {
    pub pa: usize,
    pub page_num: usize,
}

impl PageFrame {
    pub fn new(pa: usize, page_num: usize) -> Self {
        assert_eq!(pa % PAGE_SIZE, 0);
        PageFrame { pa, page_num }
    }

    pub fn alloc_pages(page_num: usize) -> Result<Self, AllocError> {
        match mem_heap_alloc(page_num, false) {
            Ok(pa) => Ok(Self::new(pa, page_num)),
            Err(err) => Err(err),
        }
    }

    pub fn pa(&self) -> usize {
        self.pa
    }

    pub fn zero(&self) {
        memset_safe(self.pa as *mut u8, 0, PAGE_SIZE);
    }

    pub fn as_slice<T>(&self) -> &'static [T] {
        if trace() && self.pa() < 0x1000 {
            panic!("illegal addr {:x}", self.pa());
        }
        unsafe { core::slice::from_raw_parts(self.pa as *const T, PAGE_SIZE / core::mem::size_of::<T>()) }
    }

    pub fn as_mut_slice<T>(&self) -> &'static mut [T] {
        if trace() && self.pa() < 0x1000 {
            panic!("illegal addr {:x}", self.pa());
        }
        unsafe { core::slice::from_raw_parts_mut(self.pa as *mut T, PAGE_SIZE / core::mem::size_of::<T>()) }
    }
}

impl Drop for PageFrame {
    fn drop(&mut self) {
        mem_heap_free(self.pa, 1);
    }
}

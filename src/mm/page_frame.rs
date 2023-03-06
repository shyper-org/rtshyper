use core::alloc::{GlobalAlloc, Layout};

use crate::arch::PAGE_SIZE;
use crate::kernel::AllocError;
use crate::lib::{memset_safe, trace};

use super::HEAP_ALLOCATOR;

#[derive(Debug)]
pub struct PageFrame {
    pub pa: usize,
    pub page_num: usize,
}

#[allow(dead_code)]
impl PageFrame {
    pub fn new(pa: usize, page_num: usize) -> Self {
        assert_eq!(pa % PAGE_SIZE, 0);
        PageFrame { pa, page_num }
    }

    pub fn alloc_pages(page_num: usize) -> Result<Self, AllocError> {
        match Layout::from_size_align(page_num * PAGE_SIZE, PAGE_SIZE) {
            Ok(layout) => {
                let pa = unsafe { HEAP_ALLOCATOR.alloc(layout) };
                memset_safe(pa, 0, PAGE_SIZE);
                let pa = pa as usize;
                // println!(">>> alloc page frame {:#x}, {}", pa, page_num);
                Ok(Self::new(pa, page_num))
            }
            Err(err) => {
                error!("alloc_pages: Layout error {}", err);
                Err(AllocError::OutOfFrame)
            }
        }
    }

    pub fn pa(&self) -> usize {
        self.pa
    }

    pub fn hva(&self) -> usize {
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
        // println!("<<< free page frame {:#x}, {}", self.pa, self.page_num);
        let layout = Layout::from_size_align(self.page_num * PAGE_SIZE, PAGE_SIZE).unwrap();
        unsafe { HEAP_ALLOCATOR.dealloc(self.pa as *mut _, layout) }
    }
}

use core::alloc::{GlobalAlloc, Layout};

use crate::arch::PAGE_SIZE;
use crate::kernel::{current_cpu, AllocError};
use crate::util::memset_safe;

use super::HEAP_ALLOCATOR;

#[derive(Debug)]
pub struct PageFrame {
    pub hva: usize,
    pub page_num: usize,
    pub pa: usize,
}

assert_not_impl_any!(PageFrame: Clone);

#[allow(dead_code)]
impl PageFrame {
    pub fn new(hva: usize, page_num: usize) -> Self {
        Self {
            hva,
            page_num,
            pa: current_cpu().pt().ipa2pa(hva).unwrap(),
        }
    }

    pub fn alloc_pages(page_num: usize) -> Result<Self, AllocError> {
        if page_num == 0 {
            return Err(AllocError::AllocZeroPage);
        }
        match Layout::from_size_align(page_num * PAGE_SIZE, PAGE_SIZE) {
            Ok(layout) => {
                let hva = unsafe { HEAP_ALLOCATOR.alloc_zeroed(layout) };
                if hva.is_null() || hva as usize & (PAGE_SIZE - 1) != 0 {
                    panic!("alloc_pages: get wrong ptr {hva:#p}, layout = {:?}", layout);
                }
                memset_safe(hva, 0, PAGE_SIZE);
                let hva = hva as usize;
                Ok(Self::new(hva, page_num))
            }
            Err(err) => {
                error!("alloc_pages: Layout error {}", err);
                Err(AllocError::OutOfFrame(page_num))
            }
        }
    }

    pub fn pa(&self) -> usize {
        self.pa
    }

    pub fn hva(&self) -> usize {
        self.hva
    }

    pub fn zero(&self) {
        memset_safe(self.hva as *mut u8, 0, PAGE_SIZE);
    }
}

impl Drop for PageFrame {
    fn drop(&mut self) {
        trace!("<<< free page frame {:#x}, {}", self.pa, self.page_num);
        let layout = Layout::from_size_align(self.page_num * PAGE_SIZE, PAGE_SIZE).unwrap();
        unsafe { HEAP_ALLOCATOR.dealloc(self.hva as *mut _, layout) }
    }
}

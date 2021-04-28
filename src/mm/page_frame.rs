use rlibc::memset;

use crate::arch::PAGE_SIZE;

#[derive(Debug)]
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

  pub fn zero(&self) {
    unsafe {
      memset(self.pa as *mut u8, 0, PAGE_SIZE);
    }
  }

  pub fn as_slice<T>(&self) -> &'static [T] {
    unsafe {
      core::slice::from_raw_parts(self.pa as *const T, PAGE_SIZE / core::mem::size_of::<T>())
    }
  }

  pub fn as_mut_slice<T>(&self) -> &'static mut [T] {
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

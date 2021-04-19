use alloc::vec::Vec;
use core::ops::Range;

use spin::Mutex;

use crate::arch::*;
use crate::mm::PageFrame;

use self::Error::*;

#[derive(Copy, Clone, Debug)]
pub enum Error {
  OutOfFrame,
  FreeNotAllocated
}

struct PagePool {
  free: Vec<usize>,
  allocated: Vec<usize>,
}

pub trait PagePoolTrait {
  fn init(&mut self, range: Range<usize>);
  fn allocate(&mut self) -> Result<PageFrame, Error>;
  fn free(&mut self, pa: usize) -> Result<(), Error>;
}

impl PagePoolTrait for PagePool {
  fn init(&mut self, range: Range<usize>) {
    assert_eq!(range.start % PAGE_SIZE, 0);
    assert_eq!(range.end % PAGE_SIZE, 0);
    for pa in range.step_by(PAGE_SIZE) {
      self.free.push(pa);
    }
  }

  fn allocate(&mut self) -> Result<PageFrame, Error> {
    if let Some(pa) = self.free.pop() {
      self.allocated.push(pa);
      Ok(PageFrame::new(pa))
    } else {
      Err(OutOfFrame)
    }
  }

  fn free(&mut self, pa: usize) -> Result<(), Error> {
    if !self.allocated.contains(&pa) {
      Err(FreeNotAllocated)
    } else {
      self.allocated.retain(|p| { *p != pa });
      self.free.push(pa);
      Ok(())
    }
  }

}


static PAGE_POOL: Mutex<PagePool> = Mutex::new(PagePool {
  free: Vec::new(),
  allocated: Vec::new(),
});

pub fn init() {
  let range = super::config::paged_range();
  let mut pool = PAGE_POOL.lock();
  pool.init(range);
}

pub fn alloc() -> PageFrame {
  let mut pool = PAGE_POOL.lock();
  if let Ok(frame) = pool.allocate() {
    frame
  } else {
    panic!("page_pool: alloc failed")
  }
}

pub fn try_alloc() -> Result<PageFrame, Error> {
  let mut pool = PAGE_POOL.lock();
  let r = pool.allocate();
  r
}

pub fn free(pa: usize) -> Result<(), Error> {
  let mut pool = PAGE_POOL.lock();
  pool.free(pa)
}
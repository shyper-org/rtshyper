use alloc::vec::Vec;

use spin::Mutex;

use crate::arch::PAGE_SIZE;
use crate::lib::{BitAlloc, BitAlloc4K, BitAlloc64K, BitMap};
use crate::lib::memset_safe;
use crate::mm::PageFrame;

use super::AllocError;

const TOTAL_MEM_REGION_MAX: usize = 16;

#[derive(Copy, Clone, Eq, Debug)]
pub struct MemRegion {
    pub base: usize,
    pub size: usize,
    pub free: usize,
    pub last: usize,
}

impl PartialEq for MemRegion {
    fn eq(&self, other: &Self) -> bool {
        self.base == other.base && self.size == other.size && self.free == other.free && self.last == other.last
    }
}

impl MemRegion {
    pub const fn new() -> MemRegion {
        MemRegion {
            base: 0,
            size: 0,
            free: 0,
            last: 0,
        }
    }

    pub fn init(&mut self, base: usize, size: usize, free: usize, last: usize) {
        self.base = base;
        self.size = size;
        self.free = free;
        self.last = last;
    }
}

pub struct HeapRegion {
    pub map: BitMap<BitAlloc4K>,
    pub region: MemRegion,
}

impl HeapRegion {
    pub fn region_init(&mut self, base: usize, size: usize, free: usize, last: usize) {
        self.region.init(base, size, free, last);
    }

    pub fn alloc_page(&mut self) -> Result<PageFrame, AllocError> {
        let mut bit: usize = self.region.size;

        if self.map.get(self.region.last) == 0 {
            bit = self.region.last;
        } else {
            for i in 0..self.region.size {
                if self.map.get(i) == 0 {
                    bit = i;
                    break;
                }
            }
        }

        if bit == self.region.size {
            println!(
                "alloc_page: allocate {} pages failed (heap_base 0x{:x} remain {} total {})",
                1, self.region.base, self.region.free, self.region.size
            );
            return Err(AllocError::OutOfFrame);
        }

        if bit < self.region.size - 1 {
            self.region.last = bit + 1;
        } else {
            self.region.last = 0;
        }
        self.map.set(bit);
        self.region.free -= 1;

        let addr = self.region.base + bit * PAGE_SIZE;
        // println!("alloc page addr 0x{:x}", addr);
        memset_safe(addr as *mut u8, 0, PAGE_SIZE);
        return Ok(PageFrame::new(addr));
    }

    pub fn alloc_pages(&mut self, size: usize) -> Result<PageFrame, AllocError> {
        let mut bit: usize = self.region.size;
        let mut count: usize = 0;
        for i in 0..self.region.size {
            if count >= size {
                bit = i - count;
                break;
            }

            if self.map.get(i) == 0 {
                count += 1;
            } else {
                count = 0;
            }
        }

        if bit == self.region.size {
            println!(
                "alloc_page: allocate {} pages failed (heap_base 0x{:x} remain {} total {})",
                size, self.region.base, self.region.free, self.region.size
            );
            return Err(AllocError::OutOfFrame);
        }

        for i in bit..bit + size {
            self.map.set(i);
        }
        self.region.free -= size;
        if bit + size < self.region.size {
            self.region.last = bit + size;
        } else {
            self.region.last = 0;
        }

        let addr = self.region.base + bit * PAGE_SIZE;
        memset_safe(addr as *mut u8, 0, size * PAGE_SIZE);
        return Ok(PageFrame::new(addr));
    }

    pub fn free_page(&mut self, base: usize) -> bool {
        use crate::lib::range_in_range;
        if !range_in_range(base, PAGE_SIZE, self.region.base, self.region.size * PAGE_SIZE) {
            panic!(
                "free_page: out of range (addr 0x{:x} page num {} heap base 0x{:x} heap size 0x{:x})",
                base,
                1,
                self.region.base,
                self.region.size * PAGE_SIZE
            );
            // return false;
        }

        let page_idx = (base - self.region.base) / PAGE_SIZE;
        self.map.clear(page_idx);

        self.region.free += 1;
        self.region.last = page_idx;
        return true;
    }
}

pub struct VmRegion {
    pub region: Vec<MemRegion>,
}

impl VmRegion {
    pub fn push(&mut self, region: MemRegion) {
        self.region.push(region);
    }
}

lazy_static! {
    pub static ref HEAP_REGION: Mutex<HeapRegion> = Mutex::new(HeapRegion {
        map: BitAlloc64K::default(),
        region: MemRegion::new(),
    });
}

pub static VM_REGION: Mutex<VmRegion> = Mutex::new(VmRegion {
    region: Vec::<MemRegion>::new(),
});

pub fn bits_to_pages(bits: usize) -> usize {
    use crate::lib::round_up;
    round_up(bits, PAGE_SIZE)
}

pub fn pa_in_heap_region(pa: usize) -> bool {
    let heap_region = HEAP_REGION.lock();
    pa > heap_region.region.base && pa < (heap_region.region.base * PAGE_SIZE + heap_region.region.size)
}

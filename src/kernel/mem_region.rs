use super::AllocError;
use crate::arch::PAGE_SIZE;
use crate::lib::{BitAlloc, BitAlloc64K, BitAlloc4K, BitMap};
use crate::mm::PageFrame;
use alloc::vec::Vec;
use rlibc::memset;
use spin::{Mutex, Once};

const TOTAL_MEM_REGION_MAX: usize = 16;

pub struct MemRegion {
    pub idx: usize,
    pub base: usize,
    pub size: usize,
    pub free: usize,
    pub last: usize,
}

impl MemRegion {
    pub const fn new() -> MemRegion {
        MemRegion {
            idx: 0,
            base: 0,
            size: 0,
            free: 0,
            last: 0,
        }
    }

    pub fn init(&mut self, idx: usize, base: usize, size: usize, free: usize, last: usize) {
        self.idx = idx;
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
    pub fn region_init(&mut self, idx: usize, base: usize, size: usize, free: usize, last: usize) {
        self.region.init(idx, base, size, free, last);
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
        unsafe {
            memset(addr as *mut u8, 0, PAGE_SIZE);
        }
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

        let addr = self.region.base + (bit + size) * PAGE_SIZE;
        println!("alloc pages addr {}", addr);
        unsafe {
            memset(addr as *mut u8, 0, size * PAGE_SIZE);
        }
        return Ok(PageFrame::new(addr));
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
    pub static ref HEAPREGION: Mutex<HeapRegion> = Mutex::new(HeapRegion {
        map: BitAlloc64K::default(),
        region: MemRegion::new(),
    });
}

pub static VMREGION: Mutex<VmRegion> = Mutex::new(VmRegion {
    region: Vec::<MemRegion>::new(),
});

pub fn bits_to_pages(bits: usize) -> usize {
    use crate::arch::PAGE_SIZE;
    use crate::lib::round_up;
    round_up(bits, PAGE_SIZE)
}

// pub fn heap_size_to_bitmap_pages(bits: usize) -> usize {
//     use crate::lib::round_up;
//     // round_up(round_up(bits, ))
// }

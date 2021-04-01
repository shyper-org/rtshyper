use crate::lib::{BitAlloc, BitAlloc16, BitAlloc256, BitMap};
use alloc::vec::Vec;
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
    pub map: BitMap<BitAlloc16>,
    pub region: MemRegion,
}

impl HeapRegion {
    pub fn region_init(&mut self, idx: usize, base: usize, size: usize, free: usize, last: usize) {
        self.region.init(idx, base, size, free, last);
    }
}

pub struct VmRegion {
    region: Vec<MemRegion>,
}

impl VmRegion {
    pub fn push(&mut self, region: MemRegion) {
        self.region.push(region);
    }
}

lazy_static! {
    pub static ref HEAPREGION: Mutex<HeapRegion> = Mutex::new(HeapRegion {
        map: BitAlloc256::default(),
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

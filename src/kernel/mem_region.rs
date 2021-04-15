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

    pub fn alloc_page(&mut self) -> usize {
        let mut bit: usize = self.region.size;

        if self.map.get(self.region.last) == 0 {
            bit = self.region.last;
        } else {
            for i in 0..(self.region.size - self.region.last) {
                if self.map.get(i) == 0 {
                    bit = i;
                    break;
                }
            }
        }

        if bit == self.region.size {
            return 0;
        }

        if bit < self.region.size - 1 {
            self.region.last = bit + 1;
        } else {
            // TODO
            self.region.last = 0;
        }
        self.region.free -= 1;
        // TODO
        0
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

use alloc::vec::Vec;

use spin::Mutex;

const TOTAL_MEM_REGION_MAX: usize = 16;

#[derive(Copy, Clone, Eq, Debug)]
pub struct MemRegion {
    pub base: usize,
    pub size: usize,
    // bit
    pub free: usize,
    // bit
    pub last: usize, // bit
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

pub struct VmRegion {
    pub region: Vec<MemRegion>,
}

impl VmRegion {
    pub fn push(&mut self, region: MemRegion) {
        self.region.push(region);
    }
}

pub static VM_REGION: Mutex<VmRegion> = Mutex::new(VmRegion {
    region: Vec::<MemRegion>::new(),
});

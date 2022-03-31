// const BITMAP_SIZE: usize = 0x3000;
// const BITMAP_ATOMIC_SIZE: usize = 64;

// type BitmapAtomicType = u64;

// type BitMap([Bitmap])

use alloc::vec::Vec;

use crate::lib::{bit_extract, bit_get, bit_set};

pub trait BitAlloc {
    // The bitmap has a total of CAP bits, numbered from 0 to CAP-1 inclusively.
    const CAP: usize;

    // The default value. Workaround for `const fn new() -> Self`.
    #[allow(clippy::declare_interior_mutable_const)]
    const DEFAULT: Self;

    // Set a bit.
    fn set(&mut self, idx: usize);

    // Clear a bit
    fn clear(&mut self, idx: usize);

    // Get a bit
    fn get(&mut self, idx: usize) -> usize;

    // Whether there are free bits remaining
    // fn any(&self) -> bool;
}

// A bitmap of 4K bits
pub type BitAlloc256 = BitMap<BitAlloc16>;
// A bitmap of 4K bits
pub type BitAlloc4K = BitMap<BitAlloc256>;
// A bitmap of 64K bits
pub type BitAlloc64K = BitMap<BitAlloc4K>;
// A bitmap of 1M bits
pub type BitAlloc1M = BitMap<BitAlloc64K>;
// A bitmap of 16M bits
pub type BitAlloc16M = BitMap<BitAlloc1M>;
// A bitmap of 256M bits
pub type BitAlloc256M = BitMap<BitAlloc16M>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct BitMap<T: BitAlloc> {
    // bitset: u16,
    map: [T; 16],
}

impl<T: BitAlloc> BitMap<T> {
    pub const fn default() -> BitMap<T> {
        BitMap::<T> { map: [T::DEFAULT; 16] }
    }
}

impl<T: BitAlloc> BitAlloc for BitMap<T> {
    const CAP: usize = T::CAP * 16;

    const DEFAULT: Self = BitMap {
        // bitset: 0,
        map: [T::DEFAULT; 16],
    };

    fn set(&mut self, idx: usize) {
        let i = idx / T::CAP;
        self.map[i].set(idx % T::CAP);
        // self.0 = self.0 | (1 << i);
    }

    fn clear(&mut self, idx: usize) {
        let i = idx / T::CAP;
        self.map[i].clear(idx % T::CAP);
        // self.0 = self.0 & (!(1 << idx) & 0xffff);
    }

    fn get(&mut self, idx: usize) -> usize {
        let i = idx / T::CAP;
        self.map[i].get(idx % T::CAP)
    }
}

#[repr(C)]
// #[derive(Copy, Clone)]
pub struct BitAlloc16(u16);

impl BitAlloc16 {
    pub const fn default() -> BitAlloc16 {
        BitAlloc16(0)
    }
}

impl BitAlloc for BitAlloc16 {
    const CAP: usize = 16;
    const DEFAULT: Self = BitAlloc16(0);

    fn set(&mut self, idx: usize) {
        self.0 = self.0 | (1 << idx);
    }

    fn clear(&mut self, idx: usize) {
        self.0 = self.0 & (!(1 << idx) & 0xffff);
    }

    fn get(&mut self, idx: usize) -> usize {
        if self.0 & (1 << idx) != 0 {
            1
        } else {
            0
        }
    }
}

// flex bit map
pub struct FlexBitmap {
    len: usize,
    map: Vec<usize>,
}

impl FlexBitmap {
    pub fn new(len: usize) -> FlexBitmap {
        let mut map = vec![];
        for i in 0..(len / 64) {
            map.push(0);
        }
        FlexBitmap { len, map }
    }

    pub fn init_dirty(&mut self) {
        for i in 0..(self.len / 64) {
            self.map[i] = usize::MAX;
        }
    }

    pub fn get(&self, idx: usize) -> usize {
        if idx > self.len {
            panic!("too large idx {} for get bitmap", idx);
        }
        let val = idx / 64;
        bit_get(val, idx & 64)
    }

    pub fn set(&mut self, idx: usize, val: bool) {
        if idx > self.len {
            panic!("too large idx {} for set bitmap", idx);
        }
        if val {
            self.map[idx / 64] |= 1 << (idx & 64);
        } else {
            self.map[idx / 64] &= !(1 << (idx & 64));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitalloc16() {
        let mut bitmap = BitAlloc16::default();
        let mut value = bitmap.get(11);
        assert_eq!(value, 0);

        bitmap.set(11);
        value = bitmap.get(11);
        assert_eq!(value, 1);

        bitmap.clear(11);
        value = bitmap.get(11);
        assert_eq!(value, 0);
    }

    #[test]
    fn bitalloc256() {
        let mut bitmap = BitAlloc256::default();
        let mut value = bitmap.get(121);
        assert_eq!(value, 0);

        bitmap.set(121);
        value = bitmap.get(121);
        assert_eq!(value, 1);

        bitmap.clear(11);
        value = bitmap.get(121);
        assert_eq!(value, 0);
    }
}

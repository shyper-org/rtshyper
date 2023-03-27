use core::fmt;
use core::ops::{Add, AddAssign, Sub, SubAssign};

use crate::arch::PAGE_SIZE;

use super::vpage_allocator::MAX_PAGE_NUMBER;

#[derive(
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Default,
    Binary,
    Octal,
    LowerHex,
    UpperHex,
    BitAnd,
    BitOr,
    BitXor,
    BitAndAssign,
    BitOrAssign,
    BitXorAssign,
    Add,
    Sub,
    AddAssign,
    SubAssign,
)]
#[repr(transparent)]
pub struct VAddr(usize);

impl VAddr {
    pub const fn new(addr: usize) -> VAddr {
        VAddr(addr)
    }

    pub const fn zero() -> VAddr {
        VAddr(0)
    }

    #[inline]
    pub const fn value(&self) -> usize {
        self.0
    }

    pub const fn page_offset(&self) -> usize {
        self.0 & (PAGE_SIZE - 1)
    }

    /// Convert to mutable pointer.
    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as *mut T
    }

    /// Convert to pointer.
    pub const fn as_ptr<T>(&self) -> *const T {
        self.0 as *const T
    }
}

impl fmt::Debug for VAddr {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> fmt::Result {
        write!(f, concat!("VAddr: ", "{:#p}"), self.0 as *const u8)
    }
}

impl fmt::Display for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl fmt::Pointer for VAddr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Add<usize> for VAddr {
    type Output = VAddr;
    fn add(self, rhs: usize) -> Self::Output {
        VAddr::new(self.0.saturating_add(rhs))
    }
}

impl AddAssign<usize> for VAddr {
    fn add_assign(&mut self, rhs: usize) {
        *self = VAddr::new(self.0.saturating_add(rhs));
    }
}

impl Sub<usize> for VAddr {
    type Output = VAddr;
    fn sub(self, rhs: usize) -> Self::Output {
        VAddr::new(self.0.saturating_sub(rhs))
    }
}

impl SubAssign<usize> for VAddr {
    fn sub_assign(&mut self, rhs: usize) {
        *self = VAddr::new(self.0.saturating_sub(rhs));
    }
}

impl From<usize> for VAddr {
    fn from(addr: usize) -> Self {
        VAddr(addr)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Page {
    pub number: usize,
}

impl Page {
    pub const fn start_address(&self) -> VAddr {
        VAddr::new(self.number * PAGE_SIZE)
    }

    pub const fn number(&self) -> usize {
        self.number
    }

    pub const fn containing_address(addr: VAddr) -> Page {
        Page {
            number: addr.value() / PAGE_SIZE,
        }
    }
}

impl fmt::Debug for Page {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, concat!(stringify!(Page), "(VAddr: {:#016x})"), self.start_address())
    }
}

impl Add<usize> for Page {
    type Output = Page;
    fn add(self, rhs: usize) -> Self::Output {
        // cannot exceed max page number (which is also max frame number)
        Page {
            number: core::cmp::min(MAX_PAGE_NUMBER, self.number.saturating_add(rhs)),
        }
    }
}

impl AddAssign<usize> for Page {
    fn add_assign(&mut self, rhs: usize) {
        *self = Page {
            number: core::cmp::min(MAX_PAGE_NUMBER, self.number.saturating_add(rhs)),
        };
    }
}

impl Sub<usize> for Page {
    type Output = Page;
    fn sub(self, rhs: usize) -> Self::Output {
        Page {
            number: self.number.saturating_sub(rhs),
        }
    }
}

impl SubAssign<usize> for Page {
    fn sub_assign(&mut self, rhs: usize) {
        *self = Page {
            number: self.number.saturating_sub(rhs),
        };
    }
}

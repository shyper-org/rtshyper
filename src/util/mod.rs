pub use self::barrier::*;
pub use self::bitmap::*;
pub use self::print::*;
pub use self::time::*;
pub use self::utility::*;

mod barrier;
mod bitmap;
pub mod downcast;
pub mod logger;
mod print;
mod time;
pub mod timer_list;
#[cfg(feature = "unilib")]
pub mod unilib;
mod utility;

pub fn memcpy_safe(dest: *const u8, src: *const u8, n: usize) {
    if (dest as usize) < 0x1000 || (src as usize) < 0x1000 {
        panic!("illegal addr for memcpy s1 {:x} s2 {:x}", dest as usize, src as usize);
    }
    unsafe {
        core::ptr::copy_nonoverlapping(src, dest as *mut _, n);
    }
}

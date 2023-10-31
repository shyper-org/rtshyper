pub use self::heap::heap_expansion;
pub use self::page_frame::*;

mod heap;
mod page;
mod page_frame;
mod util;
pub mod vpage_allocator;

// Note: link-time label, see <arch>.lds
extern "C" {
    pub fn _image_start();
    pub fn _image_end();
}

pub fn init() {
    heap::heap_init();
    vpage_allocator::init();
}

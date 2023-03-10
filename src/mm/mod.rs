pub use self::heap::*;
pub use self::page_frame::*;

mod heap;
mod page_frame;

// Note: link-time label, see aarch64.lds
extern "C" {
    pub fn _image_start();
    pub fn _image_end();
}

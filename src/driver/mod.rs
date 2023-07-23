pub use self::uart::*;

#[cfg(feature = "pi4")]
mod gpio;
mod uart;

pub fn init() {
    #[cfg(feature = "pi4")]
    {
        gpio::select_function(0, 4);
        gpio::select_function(1, 4);
    }
    uart::init();
}

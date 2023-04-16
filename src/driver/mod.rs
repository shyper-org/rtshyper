pub use self::uart::*;
#[cfg(feature = "pi4")]
pub use self::gpio::*;
#[cfg(feature = "qemu")]
pub use self::virtio::*;

#[cfg(feature = "pi4")]
mod gpio;
mod uart;
#[cfg(feature = "qemu")]
#[allow(dead_code)]
mod virtio;

pub fn init() {
    #[cfg(feature = "pi4")]
    {
        gpio_select_function(0, 4);
        gpio_select_function(1, 4);
    }
    uart::init();
}

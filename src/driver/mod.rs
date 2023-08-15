#[cfg(feature = "gpio")]
mod gpio;
pub mod uart;

pub fn init() {
    #[cfg(feature = "gpio")]
    {
        gpio::select_function(0, 4);
        gpio::select_function(1, 4);
    }
    uart::init();
}

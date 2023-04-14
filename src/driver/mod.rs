#[cfg(target_arch = "aarch64")]
pub use self::aarch64::*;
#[cfg(feature = "pi4")]
pub use self::gpio::*;
#[cfg(feature = "qemu")]
pub use self::virtio::*;

#[cfg(target_arch = "aarch64")]
mod aarch64;
#[cfg(feature = "pi4")]
mod gpio;
#[cfg(feature = "qemu")]
#[allow(dead_code)]
mod virtio;

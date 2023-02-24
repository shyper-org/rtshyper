#[cfg(target_arch = "aarch64")]
pub use self::aarch64::*;
pub use self::gpio::*;
#[cfg(feature = "qemu")]
pub use self::virtio::*;

#[cfg(target_arch = "aarch64")]
mod aarch64;
mod gpio;
#[cfg(feature = "qemu")]
mod virtio;

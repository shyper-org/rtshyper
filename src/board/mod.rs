#[cfg(feature = "pi4")]
pub use self::pi4::*;
pub use self::platform_common::*;
#[cfg(feature = "qemu")]
pub use self::qemu::*;
#[cfg(feature = "tx2")]
pub use self::tx2::*;

#[cfg(feature = "pi4")]
mod pi4;
mod platform_common;
#[cfg(feature = "qemu")]
mod qemu;
#[cfg(feature = "tx2")]
mod tx2;

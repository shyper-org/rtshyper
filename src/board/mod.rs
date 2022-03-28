pub use self::platform_common::*;
#[cfg(feature = "qemu")]
pub use self::qemu::*;
#[cfg(feature = "tx2")]
pub use self::tx2::*;

mod platform_common;
#[cfg(feature = "qemu")]
mod qemu;
#[cfg(feature = "tx2")]
mod tx2;

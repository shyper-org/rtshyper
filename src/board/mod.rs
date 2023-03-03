pub use self::platform_common::*;

#[cfg(feature = "pi4")]
pub use self::pi4::{Pi4Platform as Platform, PLAT_DESC};
#[cfg(feature = "qemu")]
pub use self::qemu::{QemuPlatform as Platform, PLAT_DESC};
#[cfg(feature = "tx2")]
pub use self::tx2::{Tx2Platform as Platform, PLAT_DESC};

mod platform_common;

#[cfg(feature = "pi4")]
mod pi4;
#[cfg(feature = "qemu")]
mod qemu;
#[cfg(feature = "tx2")]
mod tx2;

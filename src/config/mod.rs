pub use self::config::*;
#[cfg(feature = "pi4")]
pub use self::pi4_def::*;
#[cfg(feature = "qemu")]
pub use self::qemu_def::*;
#[cfg(feature = "tx2")]
pub use self::tx2_def::*;

mod config;
#[cfg(feature = "pi4")]
mod pi4_def;
#[cfg(feature = "qemu")]
mod qemu_def;
#[cfg(feature = "tx2")]
mod tx2_def;

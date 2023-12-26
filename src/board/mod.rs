pub use dev_board::{Platform, PLAT_DESC};
pub use platform_common::{PlatOperation, SchedRule};

mod platform_common;

#[cfg_attr(all(target_arch = "aarch64", feature = "tx2"), path = "./tx2.rs")]
#[cfg_attr(all(target_arch = "aarch64", feature = "qemu"), path = "./qemu.rs")]
#[cfg_attr(all(target_arch = "aarch64", feature = "pi4"), path = "./pi4.rs")]
mod dev_board;

pub mod static_config {
    include!(concat!(env!("OUT_DIR"), "/config.rs")); // CORE_NUM defined here
}

pub use platform_common::{PlatOperation, SchedRule, PLATFORM_CPU_NUM_MAX};

mod platform_common;

cfg_if::cfg_if! {
    if #[cfg(feature = "pi4")] {
        pub use self::pi4::{Pi4Platform as Platform, PLAT_DESC};
        mod pi4;
    } else if #[cfg(feature = "qemu")] {
        pub use self::qemu::{QemuPlatform as Platform, PLAT_DESC};
        mod qemu;
    } else if #[cfg(feature = "tx2")] {
        pub use self::tx2::{Tx2Platform as Platform, PLAT_DESC};
        mod tx2;
    }
}

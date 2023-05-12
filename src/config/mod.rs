pub use self::configure::*;

mod configure;

cfg_if::cfg_if! {
    if #[cfg(feature = "pi4")] {
        pub use self::pi4_def::*;
        mod pi4_def;
    } else if #[cfg(feature = "qemu")] {
        pub use self::qemu_def::*;
        mod qemu_def;
    } else if #[cfg(feature = "tx2")] {
        pub use self::tx2_def::*;
        mod tx2_def;
    }
}

cfg_if::cfg_if! {
    if #[cfg(feature = "static-config")] {
        pub use self::vm_def::*;
        #[allow(dead_code)]
        mod vm_def;
    }
}

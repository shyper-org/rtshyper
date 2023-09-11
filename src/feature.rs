use cfg_if::cfg_if;
use static_assertions::assert_cfg;

cfg_if! {
    if #[cfg(feature = "tx2")] {
        assert_cfg!(not(any(feature = "pi4", feature = "qemu")));
    } else if #[cfg(feature = "pi4")] {
        assert_cfg!(not(any(feature = "tx2", feature = "qemu")));
    } else if #[cfg(feature = "qemu")] {
        assert_cfg!(not(any(feature = "pi4", feature = "tx2")));
    } else {
        compile_error!("must provide a feature represent the platform");
    }
}

cfg_if! {
    if #[cfg(feature = "dynamic-budget")] {
        assert_cfg!(feature = "memory-reservation", "must enable memory-reservation is enable dynamic-budget");
    }
}

cfg_if! {
    if #[cfg(feature = "gpio")] {
        assert_cfg!(feature = "pi4", "gpio is only available on pi4");
    }
}

#[cfg(feature = "pa-bits-48")]
compile_error!("currently unsupported feature: pa-bits-48");

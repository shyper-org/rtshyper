#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::util::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[macro_export]
macro_rules! declare_enum_with_handler {
    (
        $enum_vis:vis enum $enum_name:ident [$array_vis:vis $array:ident => $handler_type:ty] {
            $($vis:vis $variant:ident => $handler:expr, )*
        }
    ) => {
        #[derive(Copy, Clone, Debug, PartialEq, Eq)]
        #[repr(usize)]
        $enum_vis enum $enum_name {
            $($vis $variant, )*
        }
        $array_vis static $array: &[$handler_type] = &[
            $($handler, )*
        ];
    }
}

#[macro_export]
macro_rules! atomic_read_relaxed {
    ($atomic:expr) => {
        $atomic.load(core::sync::atomic::Ordering::Relaxed)
    };
}

#[macro_export]
macro_rules! atomic_write_relaxed {
    ($atomic:expr, $val:expr) => {
        $atomic.store($val, core::sync::atomic::Ordering::Relaxed);
    };
}

#[macro_export]
macro_rules! atomic_swap_relaxed {
    ($atomic:expr, $val:expr) => {
        $atomic.swap($val, core::sync::atomic::Ordering::Relaxed)
    };
}

#[macro_export]
macro_rules! min {
    ($a:expr, $b:expr) => {
        if $a < $b {
            $a
        } else {
            $b
        }
    };
}

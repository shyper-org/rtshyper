use core::fmt::{Arguments, Write};
use spin::Mutex;

pub struct Writer;

static WRITER: Mutex<Writer> = Mutex::new(Writer);

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            crate::driver::putc(b);
        }
        Ok(())
    }
}

pub fn _print(args: Arguments) {
    // use core::fmt::Write;
    let mut lock = WRITER.lock();
    lock.write_fmt(args).unwrap();
    drop(lock);
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::lib::print::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

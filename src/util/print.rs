use core::fmt::{Arguments, Write};

use spin::Mutex;

struct Writer;

static WRITER: Mutex<Writer> = Mutex::new(Writer);

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for b in s.bytes() {
            crate::driver::uart::putc(b);
        }
        Ok(())
    }
}

pub fn _print(args: Arguments) {
    let mut lock = WRITER.lock();
    lock.write_fmt(args).unwrap();
}

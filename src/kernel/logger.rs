use log::{Level, Metadata, Record};
use log::{LevelFilter, SetLoggerError};

struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, _metadata: &Metadata) -> bool {
        true
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            // let ms = crate::lib::timer::current_ms();
            // let s = ms / 1000;
            // let ms = ms % 1000;
            // print!("[{:04}.{:03}]", s, ms);

            let level = match record.level() {
                Level::Error => "[E]",
                Level::Warn => "[W]",
                Level::Info => "[I]",
                Level::Debug => "[D]",
                Level::Trace => "[T]",
            };
            println!("{}[{}] {}", level, record.target(), record.args());
        }
    }

    fn flush(&self) {}
}

static LOGGER: SimpleLogger = SimpleLogger;

pub fn logger_init() -> Result<(), SetLoggerError> {
    log::set_logger(&LOGGER).map(|()| log::set_max_level(LevelFilter::Trace))
}

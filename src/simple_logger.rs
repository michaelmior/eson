use log::{Record, Metadata};

pub struct SimpleLogger;

impl log::Log for SimpleLogger {
  fn enabled(&self, _metadata: &Metadata) -> bool {
    true
  }

  fn log(&self, record: &Record) {
    if self.enabled(record.metadata()) {
      eprintln!("{} - {}", record.level(), record.args());
    }
  }

  fn flush(&self) { }
}

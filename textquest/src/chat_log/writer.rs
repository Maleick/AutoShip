//! Chat log writer utilities.

use std::io::{BufWriter, Write};
use std::path::Path;

pub struct LogWriter {
    writer: BufWriter<std::fs::File>,
}

impl LogWriter {
    pub fn open(path: &Path) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub fn write_line(&mut self, line: &str) -> std::io::Result<()> {
        writeln!(self.writer, "{}", line)?;
        self.writer.flush()
    }

    pub fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl Drop for LogWriter {
    fn drop(&mut self) {
        let _ = self.writer.flush();
    }
}

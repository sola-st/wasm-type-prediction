use atty::Stream;
use chrono::Local;
use indicatif::ProgressBar;
use termcolor::{BufferWriter, Color, ColorChoice, ColorSpec, WriteColor};
use std::io::{self, Write};

// FIXME not working yet
// It is quite difficult to have:
// - a progress bar at the bottom of the output (sometimes, indicatif crate)
// - colored output for log levels (termcolor crate is the only one that supports Window 7)
// - implement the standard logging facade of the log crate

// TODO add logging function to print an empty line (without level, without time etc.)

pub struct ProgressBarLog {
    progress_bar: Option<ProgressBar>,
    max_level: log::Level,
    message_output: termcolor::BufferWriter,
}

impl ProgressBarLog {
    pub fn stdout() -> Self {
        let colors = if atty::is(Stream::Stdout) {
            ColorChoice::Auto
        } else {
            ColorChoice::Never
        };
        let message_output = BufferWriter::stdout(colors);
        Self {
            progress_bar: None,
            max_level: log::Level::Info,
            message_output
        }
    }

    pub fn progress_bar(&mut self, total: u64) -> ProgressBar {
        let pb = self.progress_bar.get_or_insert(ProgressBar::new(total));
        pb.clone()
    }
}

impl log::Log for ProgressBarLog {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        metadata.level() <= self.max_level
    }

    fn log(&self, record: &log::Record) {
        if !self.enabled(record.metadata()) {
            return;
        }

        let mut buffer = self.message_output.buffer();

        let time = Local::now().to_rfc3339();
        write!(buffer, "{} ", time).unwrap();

        let mut level_style = ColorSpec::new();
        let (level_str, level_color) = match record.level() {
            log::Level::Error => {
                level_style.set_bold(true); 
                ("ERROR", Color::Red)
            },
            log::Level::Warn => ("WARN ", Color::Yellow),
            log::Level::Info => ("INFO ", Color::Green),
            log::Level::Debug => ("DEBUG", Color::Blue),
            log::Level::Trace => ("TRACE", Color::Magenta),
        };
        level_style.set_fg(Some(level_color));
        buffer.set_color(&level_style).unwrap();
        buffer.write(level_str.as_bytes()).unwrap();
        buffer.reset().unwrap();
        
        write!(buffer, " {}", record.args()).unwrap();

        self.message_output.print(&buffer).unwrap();
    }

    fn flush(&self) {}
}
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl LogLevel {
    pub fn as_str(self) -> &'static str {
        match self {
            LogLevel::Trace => "Trace",
            LogLevel::Debug => "Debug",
            LogLevel::Info => "Info",
            LogLevel::Warn => "Warn",
            LogLevel::Error => "Error",
        }
    }
}

pub struct Log;

macro_rules! log_method {
    ($name:ident, $level:expr) => {
        pub fn $name(tag: &str, text: &str) {
            Self::vlog($level, tag, text);
        }
    };
}

impl Log {
    pub fn vlog(level: LogLevel, tag: &str, text: &str) {
        let now: chrono::DateTime<chrono::Local> = std::time::SystemTime::now().into();
        let timestamp = now.format("%H:%M:%S");
        eprintln!("{} {:<5} [{}] {}", timestamp, level.as_str(), tag, text);
    }

    pub fn log(level: LogLevel, tag: &str, text: &str) {
        Self::vlog(level, tag, text);
    }

    log_method!(trace, LogLevel::Trace);
    log_method!(debug, LogLevel::Debug);
    log_method!(info, LogLevel::Info);
    log_method!(warn, LogLevel::Warn);
    log_method!(error, LogLevel::Error);
}

pub fn trace(tag: &str, text: impl fmt::Display) { Log::vlog(LogLevel::Trace, tag, &text.to_string()); }
pub fn debug(tag: &str, text: impl fmt::Display) { Log::vlog(LogLevel::Debug, tag, &text.to_string()); }
pub fn info(tag: &str, text: impl fmt::Display) { Log::vlog(LogLevel::Info, tag, &text.to_string()); }
pub fn warn(tag: &str, text: impl fmt::Display) { Log::vlog(LogLevel::Warn, tag, &text.to_string()); }
pub fn error(tag: &str, text: impl fmt::Display) { Log::vlog(LogLevel::Error, tag, &text.to_string()); }

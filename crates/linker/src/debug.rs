pub fn linker_log(prio: i32, _fmt: &str, args: std::fmt::Arguments<'_>) {
    let msg = std::fmt::format(args);
    log::log!(log::Level::Debug, "[linker/prio={}] {}", prio, msg);
}

#[macro_export]
macro_rules! linker_log {
    ($prio:expr, $fmt:literal $(, $arg:expr)*) => {
        $crate::debug::linker_log($prio, $fmt, format_args!($fmt $(, $arg)*))
    };
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_linker_log_macro() {
        linker_log!(4, "test message");
        linker_log!(3, "test with arg: {}", "value");
        linker_log!(5, "test with multiple: {} {}", 42, "hello");
    }
}

//! Rust port of `hybris_android_log_hook.cpp`.
//! The Android-priority → LogLevel map is single-sourced here; the varargs
//! `__android_log_{print,vprint,assert}` entry points remain in a tiny C++
//! shim (`android_log_varargs.cpp`) because stable Rust cannot define `...`
//! extern "C" functions. The non-varargs `__android_log_write` lives in the
//! client crate and reuses this map.

/// Android log priorities (android/log.h) — values fixed by the ABI.
pub const ANDROID_LOG_UNKNOWN: i32 = 0;
pub const ANDROID_LOG_DEFAULT: i32 = 1;
pub const ANDROID_LOG_VERBOSE: i32 = 2;
pub const ANDROID_LOG_DEBUG: i32 = 3;
pub const ANDROID_LOG_INFO: i32 = 4;
pub const ANDROID_LOG_WARN: i32 = 5;
pub const ANDROID_LOG_ERROR: i32 = 6;
pub const ANDROID_LOG_FATAL: i32 = 7;
pub const ANDROID_LOG_SILENT: i32 = 8;

/// Maps an Android log priority to a LogLevel discriminant
/// (0=Trace, 1=Debug, 2=Info, 3=Warn, 4=Error — matches both the util and C++
/// `LogLevel` enums). Faithful port of `convertAndroidLogLevel`.
pub fn convert_android_log_level(level: i32) -> i32 {
    if level <= ANDROID_LOG_VERBOSE {
        return 0;
    }
    match level {
        ANDROID_LOG_DEBUG => 1,
        ANDROID_LOG_INFO => 2,
        ANDROID_LOG_WARN => 3,
        _ => 4,
    }
}

/// FFI for the C++ varargs shim and the client `__android_log_write`.
#[no_mangle]
pub extern "C" fn mc_android_convert_log_level(level: i32) -> i32 {
    convert_android_log_level(level)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_all_priorities() {
        assert_eq!(convert_android_log_level(ANDROID_LOG_UNKNOWN), 0);
        assert_eq!(convert_android_log_level(ANDROID_LOG_DEFAULT), 0);
        assert_eq!(convert_android_log_level(ANDROID_LOG_VERBOSE), 0);
        assert_eq!(convert_android_log_level(ANDROID_LOG_DEBUG), 1);
        assert_eq!(convert_android_log_level(ANDROID_LOG_INFO), 2);
        assert_eq!(convert_android_log_level(ANDROID_LOG_WARN), 3);
        assert_eq!(convert_android_log_level(ANDROID_LOG_ERROR), 4);
        assert_eq!(convert_android_log_level(ANDROID_LOG_FATAL), 4);
        assert_eq!(convert_android_log_level(ANDROID_LOG_SILENT), 4);
    }

    #[test]
    fn out_of_range_is_trace_or_error() {
        assert_eq!(convert_android_log_level(-1), 0);
        assert_eq!(convert_android_log_level(100), 4);
    }
}

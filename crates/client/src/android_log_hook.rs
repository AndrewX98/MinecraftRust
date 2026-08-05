//! Rust port of `__android_log_write` (hybris_android_log_hook.cpp) — the only
//! non-varargs liblog entry point. The varargs siblings live in the C++ shim
//! `android_log_varargs.cpp`; all four are registered as the `liblog.so` stub
//! symbols by `capi.cpp`.

use std::ffi::{c_char, c_int, CStr};

#[no_mangle]
pub unsafe extern "C" fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) {
    if tag.is_null() || text.is_null() {
        return;
    }
    let tag = CStr::from_ptr(tag).to_string_lossy().into_owned();
    let text = CStr::from_ptr(text).to_string_lossy().into_owned();
    let level = match corelib::android_log_hook::convert_android_log_level(prio) {
        0 => util::logger::LogLevel::Trace,
        1 => util::logger::LogLevel::Debug,
        2 => util::logger::LogLevel::Info,
        3 => util::logger::LogLevel::Warn,
        _ => util::logger::LogLevel::Error,
    };
    util::logger::Log::log(level, &tag, &text);
}

//! Rust port of the liblog entry points (hybris_android_log_hook.cpp). The
//! three varargs siblings previously lived in the C++ shim
//! `android_log_varargs.cpp`; they were ported here once the crate moved to
//! nightly and enabled `#![feature(c_variadic)]`. All four are registered as
//! the `liblog.so` stub symbols by `capi.rs`.

use std::ffi::{c_char, c_int, c_void, CStr};

fn log_android_level(prio: c_int) -> util::logger::LogLevel {
    match corelib::android_log_hook::convert_android_log_level(prio) {
        0 => util::logger::LogLevel::Trace,
        1 => util::logger::LogLevel::Debug,
        2 => util::logger::LogLevel::Info,
        3 => util::logger::LogLevel::Warn,
        _ => util::logger::LogLevel::Error,
    }
}

/// Format a `va_list` (a `*mut c_void` on x86_64) via the libc-shim
/// `vsnprintf`, mirroring the C++ `Log::vlog` behaviour. Returns up to 1023
/// formatted bytes.
pub(crate) unsafe fn format_va_list(fmt: *const c_char, ap: *mut c_void) -> String {
    let mut buf = [0i8; 1024];
    let n = libc_shim::stdio::vsnprintf(buf.as_mut_ptr(), buf.len(), fmt, ap);
    if n <= 0 {
        return String::new();
    }
    let len = (n as usize).min(buf.len() - 1);
    String::from_utf8_lossy(std::slice::from_raw_parts(buf.as_ptr() as *const u8, len)).into_owned()
}

#[no_mangle]
pub unsafe extern "C" fn __android_log_write(prio: c_int, tag: *const c_char, text: *const c_char) {
    if tag.is_null() || text.is_null() {
        return;
    }
    let tag = CStr::from_ptr(tag).to_string_lossy().into_owned();
    let text = CStr::from_ptr(text).to_string_lossy().into_owned();
    util::logger::Log::log(log_android_level(prio), &tag, &text);
}

#[no_mangle]
pub unsafe extern "C" fn __android_log_vprint(
    prio: c_int,
    tag: *const c_char,
    fmt: *const c_char,
    args: *mut c_void,
) {
    if tag.is_null() || fmt.is_null() || args.is_null() {
        return;
    }
    let tag = CStr::from_ptr(tag).to_string_lossy().into_owned();
    util::logger::Log::log(log_android_level(prio), &tag, &format_va_list(fmt, args));
}

#[no_mangle]
pub unsafe extern "C" fn __android_log_print(
    prio: c_int,
    tag: *const c_char,
    fmt: *const c_char,
    args: ...,
) {
    if tag.is_null() || fmt.is_null() {
        return;
    }
    let ap = &args as *const std::ffi::VaList as *mut c_void;
    __android_log_vprint(prio, tag, fmt, ap);
}

#[no_mangle]
pub unsafe extern "C" fn __android_log_assert(
    cond: *const c_char,
    tag: *const c_char,
    fmt: *const c_char,
    args: ...,
) {
    let level = util::logger::LogLevel::Error;
    if !fmt.is_null() && !tag.is_null() {
        let ap = &args as *const std::ffi::VaList as *mut c_void;
        let tag = CStr::from_ptr(tag).to_string_lossy().into_owned();
        util::logger::Log::log(level, &tag, &format_va_list(fmt, ap));
    } else if !tag.is_null() {
        let cond = if cond.is_null() {
            String::new()
        } else {
            CStr::from_ptr(cond).to_string_lossy().into_owned()
        };
        let tag = CStr::from_ptr(tag).to_string_lossy().into_owned();
        util::logger::Log::log(level, &tag, &format!("Assertion failed: {}", cond));
    }
    std::process::abort();
}

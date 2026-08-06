//! Rust replacement for the C++ `logger_stub.cpp`. The C++ `Log::vlog`
//! static member (mangled `_ZN3Log4vlogE8LogLevelPKcS2_P13__va_list_tag`) is
//! still called by remaining C++ sources via the inline `Log::trace/debug/...`
//! macros in `include/logger/log.h`. It formats a `va_list` (a `__va_list_tag*`
//! on x86_64) with `vsnprintf`, strips trailing newlines, and forwards to the
//! Rust `mcpelauncher_log_vlog` (which maps the int level → `LogLevel`).

use std::ffi::{c_char, c_int, c_void};

#[export_name = "_ZN3Log4vlogE8LogLevelPKcS2_P13__va_list_tag"]
pub unsafe extern "C" fn log_vlog(level: c_int, tag: *const c_char, text: *const c_char, args: *mut c_void) {
    if tag.is_null() || text.is_null() || args.is_null() {
        return;
    }
    let mut buffer = [0i8; 4096];
    let len = libc_shim::stdio::vsnprintf(buffer.as_mut_ptr(), buffer.len(), text, args);
    let mut len = len.max(0) as usize;
    if len > buffer.len() {
        len = buffer.len();
    }
    while len > 0 && (buffer[len - 1] == b'\r' as i8 || buffer[len - 1] == b'\n' as i8) {
        len -= 1;
    }
    buffer[len] = 0;
    crate::rust_bridge::mcpelauncher_log_vlog(level, tag, buffer.as_ptr());
}

//! macOS placeholder for `game_window.rs` (X11/EGL window owner).
//!
//! Phase 4 of docs/PORT_MACOS.md replaces this with the real Cocoa/GLFW
//! backend. The exported surface matches the Linux module exactly so the rest
//! of the crate links unchanged; every entry point logs and returns safe
//! defaults (null token, default size).

#![allow(unused)]

use std::ffi::{c_char, c_void};

fn unimplemented(what: &str) {
    log::error!("[game_window-macos] {} not implemented yet (docs/PORT_MACOS.md Phase 4)", what);
}

#[no_mangle]
pub unsafe extern "C" fn mc_get_window_token() -> *mut c_void {
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn mc_create_default_window() -> *mut c_void {
    unimplemented("mc_create_default_window");
    std::ptr::null_mut()
}

#[no_mangle]
pub unsafe extern "C" fn mc_window_show(_w: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_window_poll_events(_w: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_window_start_text_input(_w: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_window_stop_text_input(_w: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn game_window_make_current(_w: *mut c_void, _active: i32) {}

#[no_mangle]
pub unsafe extern "C" fn game_window_swap_buffers(_w: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn game_window_get_size(_w: *mut c_void, out_w: *mut i32, out_h: *mut i32) {
    if !out_w.is_null() { *out_w = 1200; }
    if !out_h.is_null() { *out_h = 800; }
}

#[no_mangle]
pub unsafe extern "C" fn mc_get_window_size(out_w: *mut i32, out_h: *mut i32) {
    if !out_w.is_null() { *out_w = 1200; }
    if !out_h.is_null() { *out_h = 800; }
}

#[no_mangle]
pub unsafe extern "C" fn mc_set_clipboard_text(_text: *const c_char) {
    unimplemented("mc_set_clipboard_text");
}

#[no_mangle]
pub extern "C" fn mc_get_key_from_key_code(_code: i32, _meta_state: i32) -> u32 {
    0
}

/// Full window + FakeEGL/GL setup (Linux path creates the eglut window and
/// relocates real GLES2 symbols). Stubbed until the Phase 4 backend exists.
#[no_mangle]
pub unsafe extern "C" fn mc_create_window_and_setup_graphics() {
    unimplemented("mc_create_window_and_setup_graphics");
}

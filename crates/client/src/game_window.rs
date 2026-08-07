//! Rust replacement for the deleted `mcpelauncher-gamewindow` C++ lib.
//!
//! Rust eglut owns the single X11/EGL window (`eglutCreateWindow`). The "window
//! token" handed to the game (doubling as its `ANativeWindow`/`GameWindow*` and
//! as the FakeEGL surface handle) is the eglut X11 window id. Every token-based
//! helper (`game_window_make_current`, `game_window_swap_buffers`, …) forwards to
//! the Rust eglut singleton — the token value itself is never dereferenced.

use std::ffi::{c_char, c_void, CString};

use crate::eglut::compat::{
    eglutCreateWindow, eglutGetWindowHandle, eglutInit, eglutInitAPIMask, eglutInitWindowSize,
    eglutInitX11ClassInstanceName, eglutScreenHeight, eglutScreenWidth,
};
use crate::eglut::mouse::eglutSetClipboardText;
use crate::eglut::window::{
    eglutGetWindowSize, eglutMakeCurrent, eglutShowWindow, eglutSwapBuffers,
};
use crate::eglut::event::eglutPollEvents;
use crate::eglut::state::EGLUT_OPENGL_ES2_BIT;
use crate::rust_bridge::fake_egl::{fake_egl_get_proc_address, fake_egl_install_library};

/// The window token, i.e. the value the game treats as its ANativeWindow /
/// GameWindow*. This is the eglut X11 window id (stable, non-null once the
/// window exists). `mc_get_window_token` returns null before window creation so
/// `FakeLooper::prepare` falls back to `mc_create_default_window`.
fn window_token() -> *mut c_void {
    let xwin = unsafe { eglutGetWindowHandle() };
    xwin as usize as *mut c_void
}

/// Creates the eglut window (used by both the primary and fallback paths),
/// mirroring the old C++ `EGLUTWindowManager` ctor + `EGLUTWindow` ctor:
/// X11 class-instance name from `/proc/self/exe`, display init, then a window
/// sized to the screen with the GLES2 API mask. Returns the window token.
/// After this, eglut's `current_window` is populated so
/// `window_callbacks_register` can attach the Rust input trampolines.
unsafe fn create_window(title: &str) -> *mut c_void {
    let exe = std::fs::read_link("/proc/self/exe").unwrap_or_default();
    let class = exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("mcpelauncher")
        .to_string();
    let class_c = CString::new(class).unwrap();
    eglutInitX11ClassInstanceName(class_c.as_ptr());
    eglutInit(0, std::ptr::null_mut());
    let win_w = eglutScreenWidth();
    let win_h = eglutScreenHeight();
    eglutInitWindowSize(win_w, win_h);
    eglutInitAPIMask(EGLUT_OPENGL_ES2_BIT);
    let title = CString::new(title).unwrap();
    eglutCreateWindow(title.as_ptr());
    window_token()
}

// ============================================================
// Token / window helpers (previously C++ `GameWindow` method thunks)
// ============================================================

/// Returns the process-lifetime window token, or null if no window exists yet.
#[no_mangle]
pub unsafe extern "C" fn mc_get_window_token() -> *mut c_void {
    window_token()
}

/// Fallback path for `FakeLooper::prepare` when `mc_create_window_and_setup_graphics`
/// was not run: loads gamepad mappings, creates the eglut window, and installs
/// the FakeEGL GL overrides (mirrors the old C++ `mc_create_default_window`).
#[no_mangle]
pub unsafe extern "C" fn mc_create_default_window() -> *mut c_void {
    log::info!("Launcher: Loading gamepad mappings");
    crate::window_callbacks::window_callbacks_load_gamepad_mappings();
    log::info!("Launcher: Creating window");
    let token = create_window("Minecraft");
    fake_egl_setup_gl_overrides();
    token
}

#[no_mangle]
pub unsafe extern "C" fn mc_window_show(_w: *mut c_void) {
    eglutShowWindow();
}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_window_poll_events(_w: *mut c_void) {
    eglutPollEvents();
}

/// C++ `GameWindow::startTextInput` was a no-op (base class default).
#[no_mangle]
pub unsafe extern "C" fn fake_looper_window_start_text_input(_w: *mut c_void) {}

/// C++ `GameWindow::stopTextInput` was a no-op (base class default).
#[no_mangle]
pub unsafe extern "C" fn fake_looper_window_stop_text_input(_w: *mut c_void) {}

/// Upstream FakeEGL path: surface handle IS the window token. Binding/unbinding
/// goes through the Rust eglut singleton (`eglutMakeCurrent`, win < 0 unbinds).
#[no_mangle]
pub unsafe extern "C" fn game_window_make_current(_w: *mut c_void, active: i32) {
    eglutMakeCurrent(if active != 0 { 1 } else { -1 });
}

#[no_mangle]
pub unsafe extern "C" fn game_window_swap_buffers(_w: *mut c_void) {
    eglutSwapBuffers();
}

#[no_mangle]
pub unsafe extern "C" fn game_window_get_size(_w: *mut c_void, out_w: *mut i32, out_h: *mut i32) {
    eglutGetWindowSize(out_w, out_h);
}

/// Window-size accessor for the C++ `MainActivity::getScreenWidth/Height/…`
/// (replaces `window->getWindowSize`, `window` is now an opaque token).
#[no_mangle]
pub unsafe extern "C" fn mc_get_window_size(out_w: *mut i32, out_h: *mut i32) {
    eglutGetWindowSize(out_w, out_h);
}

/// Clipboard setter for the C++ `MainActivity::setClipboard`
/// (replaces `window->setClipboardText`).
#[no_mangle]
pub unsafe extern "C" fn mc_set_clipboard_text(text: *const c_char) {
    if !text.is_null() {
        eglutSetClipboardText(text);
    }
}

/// Key remap for the C++ `MainActivity::getKeyFromKeyCode`. The old
/// `GameWindow::getKeyFromKeyCode` base default always returned 0 (the eglut
/// window never overrode it), so this stays a constant 0.
#[no_mangle]
pub extern "C" fn mc_get_key_from_key_code(_code: i32, _meta_state: i32) -> u32 {
    0
}

// ============================================================
// Window creation + GL setup (previously C++ `mc_create_window_and_setup_graphics`)
// ============================================================

/// Rust `capi.cpp/jni_bridge_stub.cpp mc_create_window_and_setup_graphics`.
/// Creates the eglut window, then seeds FakeEGL (proc-addr, EGL library install,
/// GL overrides, saved window/display/context, release) and relocates the real
/// GLES2 symbols into the Rust linker.
#[no_mangle]
pub unsafe extern "C" fn mc_create_window_and_setup_graphics() {
    // XInitThreads is required by Mesa EGL for multi-threaded X access.
    x11::xlib::XInitThreads();
    log::info!("LAUNCHER: XInitThreads() called successfully");

    log::info!("LAUNCHER: Creating window via eglut (Rust)");
    let _token = create_window("Minecraft");
    log::info!("LAUNCHER: Window created successfully");

    fake_egl_set_proc_addr_function(real_egl_get_proc_address());
    fake_egl_install_library();
    fake_egl_setup_gl_overrides();
    fake_egl_save_current_window_handle();
    fake_egl_save_native_window(eglutGetWindowHandle());
    fake_egl_release_context();
    log::info!("LAUNCHER: FakeEGL installed");

    crate::startup::mc_relocate_glesv2_symbols(Some(fake_egl_get_proc_address));
    log::info!("LAUNCHER: Graphics setup complete");
}

/// Resolves the real libEGL `eglGetProcAddress` (the old C++
/// `GameWindowManager::getProcAddrFunc` returned the same libEGL symbol). This
/// must NOT be the FakeEGL wrapper — the wrapper locks `HOST_PROC_OVERRIDES`,
/// which `fake_egl_setup_gl_overrides` already holds while resolving symbols
/// through this function, so passing the wrapper self-deadlocks.
unsafe fn real_egl_get_proc_address() -> *mut c_void {
    let libegl = libc::dlopen(
        c"libEGL.so".as_ptr() as *const libc::c_char,
        libc::RTLD_LAZY | libc::RTLD_LOCAL,
    );
    if libegl.is_null() {
        log::warn!("LAUNCHER: libEGL.so not loadable; eglGetProcAddress resolver = null");
        return std::ptr::null_mut();
    }
    let addr = libc::dlsym(libegl, c"eglGetProcAddress".as_ptr() as *const libc::c_char);
    log::info!("LAUNCHER: real eglGetProcAddress = {:p}", addr);
    addr
}

// ============================================================
// FakeEGL Rust externs called from this module
// ============================================================

extern "C" {
    fn fake_egl_set_proc_addr_function(fn_ptr: *mut c_void);
    fn fake_egl_setup_gl_overrides();
    fn fake_egl_save_current_window_handle();
    fn fake_egl_save_native_window(window: u64);
    fn fake_egl_release_context();
}

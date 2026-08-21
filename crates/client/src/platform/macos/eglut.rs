//! macOS placeholder for the X11/EGL `eglut` module.
//!
//! The real windowing backend for macOS (Cocoa/GLFW, Phase 4 of
//! docs/PORT_MACOS.md) is not implemented yet. This stub keeps the client
//! compiling on darwin by exporting the same `#[no_mangle]` surface the rest
//! of the crate links against (`window_callbacks.rs` declares these via
//! `extern "C"`), plus the `STATE` shape `rust_bridge.rs` reads. Every entry
//! point is a logged no-op; startup will report the missing backend instead
//! of failing to link.

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals, unused)]

use std::ffi::{c_char, c_void};

pub type EGLDisplay = *mut c_void;
pub type EGLConfig = *mut c_void;
pub type EGLContext = *mut c_void;
pub type EGLSurface = *mut c_void;

pub const WINDOWED: i32 = 0;
pub const FULLSCREEN: i32 = 1;
pub const NOT_FOCUSED: i32 = 0;
pub const FOCUSED: i32 = 1;
pub const POINTER_VISIBLE: i32 = 1;
pub const POINTER_UNLOCKED: i32 = 0;
pub const POINTER_LOCKED: i32 = 1;
pub const EGLUT_KEY_PRESS: i32 = 0;
pub const EGLUT_KEY_RELEASE: i32 = 1;
pub const EGLUT_KEY_REPEAT: i32 = 2;
pub const EGLUT_MOUSE_PRESS: i32 = 0;
pub const EGLUT_MOUSE_RELEASE: i32 = 1;
pub const EGLUT_OPENGL_BIT: i32 = 0x1;
pub const EGLUT_OPENGL_ES1_BIT: i32 = 0x2;
pub const EGLUT_OPENGL_ES2_BIT: i32 = 0x4;
pub const EGLUT_OPENVG_BIT: i32 = 0x8;

pub type EGLUTidleCB = Option<unsafe extern "C" fn()>;
pub type EGLUTreshapeCB = Option<unsafe extern "C" fn(i32, i32)>;
pub type EGLUTdisplayCB = Option<unsafe extern "C" fn()>;
pub type EGLUTkeyboardCB = Option<unsafe extern "C" fn(*mut c_char, i32)>;
pub type EGLUTdropCB = Option<unsafe extern "C" fn(*const c_char)>;
pub type EGLUTspecialCB = Option<unsafe extern "C" fn(i32, i32, u32)>;
pub type EGLUTpasteCB = Option<unsafe extern "C" fn(*const c_char, i32)>;
pub type EGLUTmouseCB = Option<unsafe extern "C" fn(i32, i32)>;
pub type EGLUTmouseRawCB = Option<unsafe extern "C" fn(f64, f64)>;
pub type EGLUTmouseButtonCB = Option<unsafe extern "C" fn(i32, i32, i32, i32)>;
pub type EGLUTtouchStartCB = Option<unsafe extern "C" fn(i32, f64, f64)>;
pub type EGLUTtouchUpdateCB = Option<unsafe extern "C" fn(i32, f64, f64)>;
pub type EGLUTtouchEndCB = Option<unsafe extern "C" fn(i32, f64, f64)>;
pub type EGLUTfocusCB = Option<unsafe extern "C" fn(i32)>;
pub type EGLUTcloseCB = Option<unsafe extern "C" fn()>;

/// Only the fields read from outside this module are kept; the macOS backend
/// (Phase 4) will extend this to the full Linux `EglutWindow` shape if needed.
pub struct EglutWindow {
    pub width: i32,
    pub height: i32,
    pub context: EGLContext,
    pub surface: EGLSurface,
    pub config: EGLConfig,
}

pub struct EglutState {
    pub egl_dpy: EGLDisplay,
    pub current_window: Option<Box<EglutWindow>>,
}

pub static mut STATE: EglutState = EglutState {
    egl_dpy: std::ptr::null_mut(),
    current_window: None,
};

fn unimplemented(what: &str) {
    log::error!("[eglut-macos] {} not implemented yet (docs/PORT_MACOS.md Phase 4)", what);
}

// ── init / window lifecycle (compat.rs / window.rs counterparts) ──

#[no_mangle]
pub unsafe extern "C" fn eglutInit(argc: i32, _argv: *mut *mut c_char) { let _ = argc; unimplemented("eglutInit"); }
#[no_mangle]
pub unsafe extern "C" fn eglutInitX11ClassInstanceName(_value: *const c_char) {}
#[no_mangle]
pub unsafe extern "C" fn eglutInitWindowSize(_width: i32, _height: i32) {}
#[no_mangle]
pub unsafe extern "C" fn eglutInitAPIMask(_mask: i32) {}
#[no_mangle]
pub unsafe extern "C" fn eglutScreenWidth() -> i32 { 1200 }
#[no_mangle]
pub unsafe extern "C" fn eglutScreenHeight() -> i32 { 800 }
#[no_mangle]
pub unsafe extern "C" fn eglutCreateWindow(_title: *const c_char) -> i32 { unimplemented("eglutCreateWindow"); -1 }
#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowHandle() -> u64 { 0 }
#[no_mangle]
pub unsafe extern "C" fn eglutShowWindow() {}
#[no_mangle]
pub unsafe extern "C" fn eglutMakeCurrent(_win: i32) { unimplemented("eglutMakeCurrent"); }
#[no_mangle]
pub unsafe extern "C" fn eglutSwapBuffers() {}
#[no_mangle]
pub unsafe extern "C" fn eglutPollEvents() {}
#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowSize(w: *mut i32, h: *mut i32) {
    if !w.is_null() { *w = 1200; }
    if !h.is_null() { *h = 800; }
}
#[no_mangle]
pub unsafe extern "C" fn eglutGet(_param: i32) -> i32 { 0 }

// ── mouse / clipboard / fullscreen (mouse.rs counterparts) ──

#[no_mangle]
pub unsafe extern "C" fn eglutSetMousePointerLocked(_locked: i32) { unimplemented("eglutSetMousePointerLocked"); }
#[no_mangle]
pub unsafe extern "C" fn eglutWarpMousePointer(_x: i32, _y: i32) {}
#[no_mangle]
pub unsafe extern "C" fn eglutSetClipboardText(_text: *const c_char) { unimplemented("eglutSetClipboardText"); }
#[no_mangle]
pub unsafe extern "C" fn eglutRequestPaste() { unimplemented("eglutRequestPaste"); }
#[no_mangle]
pub unsafe extern "C" fn eglutToggleFullscreen() { unimplemented("eglutToggleFullscreen"); }

// ── callback registration (callbacks.rs counterparts — stored nowhere yet) ──

macro_rules! noop_cb_reg {
    ($name:ident, $cb:ty) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(_func: $cb) {}
    };
}

noop_cb_reg!(eglutDisplayFunc, EGLUTdisplayCB);
noop_cb_reg!(eglutReshapeFunc, EGLUTreshapeCB);
noop_cb_reg!(eglutKeyboardFunc, EGLUTkeyboardCB);
noop_cb_reg!(eglutDropFunc, EGLUTdropCB);
noop_cb_reg!(eglutSpecialFunc, EGLUTspecialCB);
noop_cb_reg!(eglutPasteFunc, EGLUTpasteCB);
noop_cb_reg!(eglutMouseFunc, EGLUTmouseCB);
noop_cb_reg!(eglutMouseRawFunc, EGLUTmouseRawCB);
noop_cb_reg!(eglutMouseButtonFunc, EGLUTmouseButtonCB);
noop_cb_reg!(eglutTouchStartFunc, EGLUTtouchStartCB);
noop_cb_reg!(eglutTouchUpdateFunc, EGLUTtouchUpdateCB);
noop_cb_reg!(eglutTouchEndFunc, EGLUTtouchEndCB);
noop_cb_reg!(eglutFocusFunc, EGLUTfocusCB);
noop_cb_reg!(eglutCloseWindowFunc, EGLUTcloseCB);
noop_cb_reg!(eglutIdleFunc, EGLUTidleCB);

#[no_mangle]
pub unsafe extern "C" fn eglutSetKeyboardState(_active: i32) {}

/// Path-compat shim: the Linux module is a directory (`eglut/`) with a
/// `state.rs` child; code references `crate::eglut::state::…`.
pub mod state {
    pub use super::*;
}

//! macOS `eglut` counterpart: callback + window-state store backing the
//! GLFW backend in `platform/macos/game_window.rs`.
//!
//! The Linux module drives X11 directly; here the module only owns the state
//! shape (`STATE`, callbacks) and the registration entry points that
//! `window_callbacks.rs` links against. Event pumping and dispatch live in
//! the GLFW backend.

#![allow(non_camel_case_types, non_snake_case, non_upper_case_globals)]

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

pub struct EglutWindow {
    pub width: i32,
    pub height: i32,
    pub context: EGLContext,
    pub surface: EGLSurface,
    pub config: EGLConfig,
    pub reshape_cb: EGLUTreshapeCB,
    pub display_cb: EGLUTdisplayCB,
    pub keyboard_cb: EGLUTkeyboardCB,
    pub drop_cb: EGLUTdropCB,
    pub special_cb: EGLUTspecialCB,
    pub paste_cb: EGLUTpasteCB,
    pub mouse_cb: EGLUTmouseCB,
    pub mouse_raw_cb: EGLUTmouseRawCB,
    pub mouse_button_cb: EGLUTmouseButtonCB,
    pub touch_start_cb: EGLUTtouchStartCB,
    pub touch_update_cb: EGLUTtouchUpdateCB,
    pub touch_end_cb: EGLUTtouchEndCB,
    pub focus_cb: EGLUTfocusCB,
    pub close_cb: EGLUTcloseCB,
    pub keyboardstate: i32,
}

impl EglutWindow {
    /// Window-shaped placeholder so callbacks can register before/after the
    /// GLFW window exists (registration order differs slightly from Linux).
    fn new(width: i32, height: i32) -> Self {
        EglutWindow {
            width,
            height,
            context: std::ptr::null_mut(),
            surface: std::ptr::null_mut(),
            config: std::ptr::null_mut(),
            reshape_cb: None,
            display_cb: None,
            keyboard_cb: None,
            drop_cb: None,
            special_cb: None,
            paste_cb: None,
            mouse_cb: None,
            mouse_raw_cb: None,
            mouse_button_cb: None,
            touch_start_cb: None,
            touch_update_cb: None,
            touch_end_cb: None,
            focus_cb: None,
            close_cb: None,
            keyboardstate: 0,
        }
    }
}

pub struct EglutState {
    // Synthetic non-null display handle; FakeEGL saves it via
    // fake_egl_save_current_window_handle (no real EGL on macOS).
    pub egl_dpy: EGLDisplay,
    pub api_mask: i32,
    pub window_fullscreen: i32,
    pub current_window: Option<Box<EglutWindow>>,
    pub idle_cb: EGLUTidleCB,
    pub redisplay: bool,
    // relative-movement bookkeeping (mirrors Linux eglut)
    pub relative_movement_enabled: bool,
    pub relative_movement_last_x: i32,
    pub relative_movement_last_y: i32,
}

pub static mut STATE: EglutState = EglutState {
    egl_dpy: std::ptr::null_mut(),
    api_mask: EGLUT_OPENGL_ES2_BIT,
    window_fullscreen: WINDOWED,
    current_window: None,
    idle_cb: None,
    redisplay: false,
    relative_movement_enabled: false,
    relative_movement_last_x: 0,
    relative_movement_last_y: 0,
};

/// Ensure a placeholder window exists (callback storage target).
pub(crate) unsafe fn ensure_window(width: i32, height: i32) -> *mut EglutWindow {
    if STATE.current_window.is_none() {
        STATE.current_window = Some(Box::new(EglutWindow::new(width, height)));
    }
    let win = STATE.current_window.as_mut().unwrap().as_mut() as *mut EglutWindow;
    if width > 0 { (*win).width = width; }
    if height > 0 { (*win).height = height; }
    win
}

// ── callback registration (same surface as eglut/callbacks.rs) ──

macro_rules! cb_reg {
    ($name:ident, $cb:ty, $field:ident) => {
        #[no_mangle]
        pub unsafe extern "C" fn $name(func: $cb) {
            ensure_window(0, 0);
            if let Some(win) = STATE.current_window.as_mut() {
                win.$field = func;
            }
        }
    };
}

cb_reg!(eglutDisplayFunc, EGLUTdisplayCB, display_cb);
cb_reg!(eglutReshapeFunc, EGLUTreshapeCB, reshape_cb);
cb_reg!(eglutKeyboardFunc, EGLUTkeyboardCB, keyboard_cb);
cb_reg!(eglutDropFunc, EGLUTdropCB, drop_cb);
cb_reg!(eglutSpecialFunc, EGLUTspecialCB, special_cb);
cb_reg!(eglutPasteFunc, EGLUTpasteCB, paste_cb);
cb_reg!(eglutMouseFunc, EGLUTmouseCB, mouse_cb);
cb_reg!(eglutMouseRawFunc, EGLUTmouseRawCB, mouse_raw_cb);
cb_reg!(eglutMouseButtonFunc, EGLUTmouseButtonCB, mouse_button_cb);
cb_reg!(eglutTouchStartFunc, EGLUTtouchStartCB, touch_start_cb);
cb_reg!(eglutTouchUpdateFunc, EGLUTtouchUpdateCB, touch_update_cb);
cb_reg!(eglutTouchEndFunc, EGLUTtouchEndCB, touch_end_cb);
cb_reg!(eglutFocusFunc, EGLUTfocusCB, focus_cb);
cb_reg!(eglutCloseWindowFunc, EGLUTcloseCB, close_cb);

#[no_mangle]
pub unsafe extern "C" fn eglutIdleFunc(func: EGLUTidleCB) {
    STATE.idle_cb = func;
}

#[no_mangle]
pub unsafe extern "C" fn eglutSetKeyboardState(active: i32) {
    ensure_window(0, 0);
    if let Some(win) = STATE.current_window.as_mut() {
        win.keyboardstate = active;
    }
}

/// Path-compat shim: the Linux module is a directory (`eglut/`) with a
/// `state.rs` child; code references `crate::eglut::state::…`.
pub mod state {
    pub use super::*;
}

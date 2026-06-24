use std::ffi::{c_char, c_void, CString};
use x11::xlib::*;

use crate::eglut::egl::*;

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

pub const WINDOWED: i32 = 0;
pub const FULLSCREEN: i32 = 1;
pub const NOT_FOCUSED: i32 = 0;
pub const FOCUSED: i32 = 1;
pub const POINTER_INVISIBLE: i32 = 0;
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
pub const EGLUT_ELAPSED_TIME: i32 = 0;
pub const EGLUT_FULLSCREEN_MODE: i32 = 1;

pub struct EglutWindow {
    pub xwin: Window,
    pub width: i32, pub height: i32, pub x: i32, pub y: i32,
    pub context: EGLContext,
    pub surface: EGLSurface,
    pub config: EGLConfig,
    pub index: i32,
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

pub struct EglutState {
    pub display: *mut Display,
    pub egl_dpy: EGLDisplay,
    pub api_mask: i32,
    pub window_width: i32,
    pub window_height: i32,
    pub window_fullscreen: i32,
    pub verbose: bool,
    pub init_time: i64,
    pub surface_type: EGLint,
    pub num_windows: i32,
    pub current_xwin: Window,
    pub idle_cb: EGLUTidleCB,
    pub redisplay: bool,
    pub xdnd_drop: Atom, pub xdnd_type_list: Atom, pub xdnd_selection: Atom,
    pub xdnd_enter: Atom, pub xdnd_position: Atom, pub xdnd_status: Atom,
    pub xdnd_leave: Atom, pub xdnd_finished: Atom, pub xdnd_action_copy: Atom,
    pub xtext_uri_list: Atom,
    pub dnd_source: i64, pub dnd_version: i64, pub dnd_format: i32,
    pub current_window: Option<Box<EglutWindow>>,
}

pub static mut STATE: EglutState = EglutState {
    display: std::ptr::null_mut(),
    egl_dpy: EGL_NO_DISPLAY,
    api_mask: EGLUT_OPENGL_ES1_BIT,
    window_width: 300, window_height: 300,
    window_fullscreen: WINDOWED,
    verbose: false, init_time: 0, surface_type: 0,
    num_windows: 0, current_xwin: 0,
    idle_cb: None, redisplay: false,
    xdnd_drop: 0, xdnd_type_list: 0, xdnd_selection: 0, xdnd_enter: 0,
    xdnd_position: 0, xdnd_status: 0, xdnd_leave: 0, xdnd_finished: 0,
    xdnd_action_copy: 0, xtext_uri_list: 0,
    dnd_source: 0, dnd_version: 0, dnd_format: 0,
    current_window: None,
};

pub static mut RELATIVE_MOVEMENT_ENABLED: bool = false;
pub static mut RELATIVE_MOVEMENT_LAST_X: i32 = 0;
pub static mut RELATIVE_MOVEMENT_LAST_Y: i32 = 0;
pub static mut RELATIVE_MOVEMENT_RAW_MODE: bool = false;
pub static mut CURSOR_GRABBED: bool = false;
pub static mut CLIPBOARD_TEXT: Option<CString> = None;
pub static mut X11_IC: *mut c_void = std::ptr::null_mut();

pub unsafe fn now_ms() -> i64 {
    let mut tv = std::mem::zeroed::<libc::timeval>();
    libc::gettimeofday(&mut tv, std::ptr::null_mut());
    tv.tv_sec as i64 * 1000 + tv.tv_usec as i64 / 1000
}

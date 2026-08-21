//! macOS windowing backend: GLFW/Cocoa + desktop OpenGL 4.1 core.
//!
//! Phase 4 of docs/PORT_MACOS.md. Mirrors the surface of the Linux
//! `game_window.rs` exactly (same `#[no_mangle]` exports) so the rest of the
//! crate links unchanged, but drives GLFW instead of X11/EGL:
//!
//! - GL symbols resolve through `glfwGetProcAddress` (desktop GL), fed to
//!   FakeEGL via `HOST_PROC_ADDR_FN`; `mc_glcorepatch_must_use_desktop_gl`
//!   returns true on this target so the game uses its core-profile path.
//! - Events are translated to the eglut callback contract: keysyms are
//!   converted to X11 values so `window_callbacks::get_key_minecraft` keeps
//!   working unmodified; modifiers use X11 mask bits.
//! - All GLFW calls go through one lock: Cocoa wants the main thread for
//!   window/event ops while make_current/swap arrive from render threads.

#![allow(non_camel_case_types, non_snake_case, unused)]

use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};
use std::sync::Mutex;

use glfw::Context;

use crate::eglut::state::{
    ensure_window, STATE, EGLUT_MOUSE_PRESS, EGLUT_MOUSE_RELEASE, FOCUSED, FULLSCREEN,
    NOT_FOCUSED, POINTER_LOCKED, POINTER_UNLOCKED, WINDOWED,
};
use crate::rust_bridge::fake_egl::{fake_egl_get_proc_address, fake_egl_install_library};

// ── state ──

struct GlfwState {
    glfw: glfw::Glfw,
    win: glfw::PWindow,
    events: glfw::GlfwReceiver<(f64, WindowEventT)>,
    fullscreen: bool,
    windowed_size: (i32, i32),
}

// GLFW handles are plain pointers; access is serialized through GLFW_LOCK.
unsafe impl Send for GlfwState {}
unsafe impl Sync for GlfwState {}

type WindowEventT = glfw::WindowEvent;

static STATE_PTR: AtomicPtr<GlfwState> = AtomicPtr::new(std::ptr::null_mut());
static GLFW_LOCK: Mutex<()> = Mutex::new(());
/// Set once `mc_create_window_and_setup_graphics` ran successfully.
static WINDOW_READY: AtomicBool = AtomicBool::new(false);

fn locked() -> Option<std::sync::MutexGuard<'static, ()>> {
    if STATE_PTR.load(Ordering::Acquire).is_null() {
        return None;
    }
    let guard = GLFW_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if STATE_PTR.load(Ordering::Acquire).is_null() {
        return None;
    }
    Some(guard)
}

fn state() -> Option<&'static mut GlfwState> {
    let ptr = STATE_PTR.load(Ordering::Acquire);
    if ptr.is_null() { None } else { Some(unsafe { &mut *ptr }) }
}

/// The window token handed to the game: the heap address of the GlfwState
/// (stable for the process lifetime, non-null once created).
fn token() -> *mut c_void {
    STATE_PTR.load(Ordering::Acquire) as *mut c_void
}

const DEFAULT_W: i32 = 1200;
const DEFAULT_H: i32 = 800;

unsafe fn create_window(title: &str) -> *mut c_void {
    let _lock = GLFW_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    if !STATE_PTR.load(Ordering::Acquire).is_null() {
        return token();
    }

    let mut glfw = match glfw::init(|err, desc| {
        log::error!("[glfw] error {:?}: {}", err, desc);
    }) {
        Ok(g) => g,
        Err(e) => {
            log::error!("[game_window-macos] glfw::init failed: {:?}", e);
            return std::ptr::null_mut();
        }
    };

    // macOS caps at GL 4.1 core; forward-compat core profile is mandatory
    glfw.window_hint(glfw::WindowHint::ContextVersion(4, 1));
    glfw.window_hint(glfw::WindowHint::OpenGlProfile(
        glfw::OpenGlProfileHint::Core,
    ));
    glfw.window_hint(glfw::WindowHint::OpenGlForwardCompat(true));

    let title_c = CString::new(title).unwrap_or_default();
    let (mut win, events) = match glfw.create_window(
        DEFAULT_W as u32,
        DEFAULT_H as u32,
        title_c.to_str().unwrap_or("Minecraft"),
        glfw::WindowMode::Windowed,
    ) {
        Some(pair) => pair,
        None => {
            log::error!("[game_window-macos] failed to create GLFW window");
            return std::ptr::null_mut();
        }
    };
    win.make_current();
    win.set_framebuffer_size_polling(true);
    win.set_cursor_pos_polling(true);
    win.set_mouse_button_polling(true);
    win.set_key_polling(true);
    win.set_char_polling(true);
    win.set_scroll_polling(true);
    win.set_focus_polling(true);
    win.set_close_polling(true);
    glfw.set_swap_interval(glfw::SwapInterval::Sync(1));

    let ptr = Box::into_raw(Box::new(GlfwState {
        glfw,
        win,
        events,
        fullscreen: false,
        windowed_size: (DEFAULT_W, DEFAULT_H),
    }));
    STATE_PTR.store(ptr, Ordering::Release);

    let st = &mut *ptr;
    let (fb_w, fb_h) = st.win.get_framebuffer_size();
    ensure_window(fb_w.max(1), fb_h.max(1));
    // Synthetic display handle so FakeEGL's saved-display fallback is non-null
    STATE.egl_dpy = 1usize as *mut c_void;
    WINDOW_READY.store(true, Ordering::Release);

    log::info!(
        "[game_window-macos] GLFW window created {}x{} (GL 4.1 core)",
        fb_w,
        fb_h
    );
    token()
}

// ============================================================
// Exported surface (matches game_window.rs)
// ============================================================

#[no_mangle]
pub unsafe extern "C" fn mc_get_window_token() -> *mut c_void {
    token()
}

#[no_mangle]
pub unsafe extern "C" fn mc_create_default_window() -> *mut c_void {
    log::info!("Launcher: Loading gamepad mappings");
    crate::window_callbacks::window_callbacks_load_gamepad_mappings();
    log::info!("Launcher: Creating window");
    create_window("Minecraft")
}

#[no_mangle]
pub unsafe extern "C" fn mc_window_show(_w: *mut c_void) {
    if let Some(lock) = locked() {
        let st = state().unwrap();
        st.win.show();
        drop(lock);
    }
}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_window_poll_events(_w: *mut c_void) {
    pump_events();
}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_window_start_text_input(_w: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_window_stop_text_input(_w: *mut c_void) {}

#[no_mangle]
pub unsafe extern "C" fn game_window_make_current(_w: *mut c_void, active: i32) {
    if let Some(lock) = GLFW_LOCK.lock().ok() {
        if active != 0 {
            if let Some(st) = state() {
                st.win.make_current();
            }
        } else {
            // unbind: GLFW allows passing no window
            if !STATE_PTR.load(Ordering::Acquire).is_null() {
                drop_context();
            }
        }
        drop(lock);
    }
}

unsafe fn drop_context() {
    // glfw crate: making a null context current is expressed by dropping the
    // current binding — Context::make_current needs a window, so use raw FFI
    extern "C" {
        fn glfwMakeContextCurrent(window: *mut c_void);
    }
    glfwMakeContextCurrent(std::ptr::null_mut());
}

#[no_mangle]
pub unsafe extern "C" fn game_window_swap_buffers(_w: *mut c_void) {
    if let Some(lock) = locked() {
        let st = state().unwrap();
        st.win.swap_buffers();
        drop(lock);
    }
}

#[no_mangle]
pub unsafe extern "C" fn game_window_get_size(_w: *mut c_void, out_w: *mut i32, out_h: *mut i32) {
    mc_get_window_size(out_w, out_h)
}

#[no_mangle]
pub unsafe extern "C" fn mc_get_window_size(out_w: *mut i32, out_h: *mut i32) {
    let mut w = DEFAULT_W;
    let mut h = DEFAULT_H;
    if let Some(lock) = locked() {
        let st = state().unwrap();
        let (fw, fh) = st.win.get_framebuffer_size();
        w = fw;
        h = fh;
        drop(lock);
    } else if let Some(win) = STATE.current_window.as_ref() {
        w = win.width;
        h = win.height;
    }
    if !out_w.is_null() { *out_w = w; }
    if !out_h.is_null() { *out_h = h; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowSize(out_w: *mut i32, out_h: *mut i32) {
    mc_get_window_size(out_w, out_h)
}

#[no_mangle]
pub unsafe extern "C" fn mc_set_clipboard_text(text: *const c_char) {
    if text.is_null() { return; }
    if let Ok(s) = CStr::from_ptr(text).to_str() {
        if let Some(lock) = locked() {
            let st = state().unwrap();
            st.win.set_clipboard_string(s);
            drop(lock);
        }
    }
}

#[no_mangle]
pub extern "C" fn mc_get_key_from_key_code(_code: i32, _meta_state: i32) -> u32 {
    0
}

// ── mouse / clipboard / fullscreen (mouse.rs counterparts) ──

#[no_mangle]
pub unsafe extern "C" fn eglutSetMousePointerLocked(locked_flag: i32) {
    STATE.relative_movement_enabled = locked_flag == POINTER_LOCKED;
    if let Some(lock) = locked() {
        let st = state().unwrap();
        st.win.set_cursor_mode(if locked_flag == POINTER_LOCKED {
            glfw::CursorMode::Disabled
        } else {
            glfw::CursorMode::Normal
        });
        drop(lock);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutWarpMousePointer(x: i32, y: i32) {
    if let Some(lock) = locked() {
        let st = state().unwrap();
        st.win.set_cursor_pos(x as f64, y as f64);
        drop(lock);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutSetClipboardText(text: *const c_char) {
    mc_set_clipboard_text(text)
}

#[no_mangle]
pub unsafe extern "C" fn eglutRequestPaste() {
    if let Some(lock) = locked() {
        let st = state().unwrap();
        let s = st.win.get_clipboard_string().unwrap_or_default();
        drop(lock);
        let cstr = CString::new(s).unwrap_or_default();
        if let Some(win) = STATE.current_window.as_ref() {
            if let Some(cb) = win.paste_cb {
                cb(cstr.as_ptr(), cstr.as_bytes().len() as i32);
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutToggleFullscreen() {
    let target_fullscreen;
    {
        let lock = match locked() { Some(l) => l, None => return };
        let st = state().unwrap();
        if st.fullscreen {
            let (w, h) = st.windowed_size;
            st.win.set_monitor(glfw::WindowMode::Windowed, 0, 0, w as u32, h as u32, Some(0));
            st.fullscreen = false;
        } else {
            // do the fullscreen switch inside the monitor-borrowing callback
            st.windowed_size = st.win.get_size();
            st.glfw.with_primary_monitor(|_, m| {
                if let Some(m) = m {
                    if let Some(mode) = m.get_video_mode() {
                        st.win.set_monitor(
                            glfw::WindowMode::FullScreen(m),
                            0,
                            0,
                            mode.width,
                            mode.height,
                            Some(mode.refresh_rate),
                        );
                        st.fullscreen = true;
                    }
                }
            });
        }
        target_fullscreen = st.fullscreen;
        drop(lock);
    }
    STATE.window_fullscreen = if target_fullscreen { FULLSCREEN } else { WINDOWED };
}

#[no_mangle]
pub unsafe extern "C" fn eglutGet(param: i32) -> i32 {
    match param {
        0 => {
            // EGLUT_ELAPSED_TIME: monotonic millis since process start
            static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
            let start = START.get_or_init(std::time::Instant::now);
            start.elapsed().as_millis() as i32
        }
        1 => STATE.window_fullscreen, // EGLUT_FULLSCREEN_MODE
        _ => 0,
    }
}

// ============================================================
// Event translation (GLFW -> eglut callback contract)
// ============================================================

/// GLFW key code -> X11 keysym, matching the `xk` constants in
/// window_callbacks.rs so `get_key_minecraft` works unmodified.
fn key_to_keysym(key: glfw::Key) -> i32 {
    use glfw::Key as K;
    match key {
        K::Space => 0x20,
        K::Apostrophe => 0x27,
        K::Comma => 0x2c,
        K::Minus => 0x2d,
        K::Period => 0x2e,
        K::Slash => 0x2f,
        K::Num0 => 0x30,
        K::Num1 => 0x31,
        K::Num2 => 0x32,
        K::Num3 => 0x33,
        K::Num4 => 0x34,
        K::Num5 => 0x35,
        K::Num6 => 0x36,
        K::Num7 => 0x37,
        K::Num8 => 0x38,
        K::Num9 => 0x39,
        K::Semicolon => 0x3b,
        K::Equal => 0x3d,
        K::A => 0x41,
        K::B => 0x42,
        K::C => 0x43,
        K::D => 0x44,
        K::E => 0x45,
        K::F => 0x46,
        K::G => 0x47,
        K::H => 0x48,
        K::I => 0x49,
        K::J => 0x4a,
        K::K => 0x4b,
        K::L => 0x4c,
        K::M => 0x4d,
        K::N => 0x4e,
        K::O => 0x4f,
        K::P => 0x50,
        K::Q => 0x51,
        K::R => 0x52,
        K::S => 0x53,
        K::T => 0x54,
        K::U => 0x55,
        K::V => 0x56,
        K::W => 0x57,
        K::X => 0x58,
        K::Y => 0x59,
        K::Z => 0x5a,
        K::LeftBracket => 0x5b,
        K::Backslash => 0x5c,
        K::RightBracket => 0x5d,
        K::GraveAccent => 0x60,
        K::World1 => 0xa2,
        K::World2 => 0xa3,
        K::Escape => 0xff1b,
        K::Enter => 0xff0d,
        K::Tab => 0xff09,
        K::Backspace => 0xff08,
        K::Insert => 0xff63,
        K::Delete => 0xffff,
        K::Right => 0xff53,
        K::Left => 0xff51,
        K::Down => 0xff54,
        K::Up => 0xff52,
        K::PageUp => 0xff55,
        K::PageDown => 0xff56,
        K::Home => 0xff50,
        K::End => 0xff57,
        K::CapsLock => 0xffe5,
        K::ScrollLock => 0xff14,
        K::NumLock => 0xff7f,
        K::PrintScreen => 0xff61,
        K::Pause => 0xff13,
        K::F1 => 0xffbe,
        K::F2 => 0xffbf,
        K::F3 => 0xffc0,
        K::F4 => 0xffc1,
        K::F5 => 0xffc2,
        K::F6 => 0xffc3,
        K::F7 => 0xffc4,
        K::F8 => 0xffc5,
        K::F9 => 0xffc6,
        K::F10 => 0xffc7,
        K::F11 => 0xffc8,
        K::F12 => 0xffc9,
        K::F13 => 0xffca,
        K::F14 => 0xffcb,
        K::F15 => 0xffcc,
        K::F16 => 0xffcd,
        K::F17 => 0xffce,
        K::F18 => 0xffcf,
        K::F19 => 0xffd0,
        K::F20 => 0xffd1,
        K::F21 => 0xffd2,
        K::F22 => 0xffd3,
        K::F23 => 0xffd4,
        K::F24 => 0xffd5,
        K::Kp0 => 0xffb0,
        K::Kp1 => 0xffb1,
        K::Kp2 => 0xffb2,
        K::Kp3 => 0xffb3,
        K::Kp4 => 0xffb4,
        K::Kp5 => 0xffb5,
        K::Kp6 => 0xffb6,
        K::Kp7 => 0xffb7,
        K::Kp8 => 0xffb8,
        K::Kp9 => 0xffb9,
        K::KpDecimal => 0xffae,
        K::KpDivide => 0xffaf,
        K::KpMultiply => 0xffaa,
        K::KpSubtract => 0xffad,
        K::KpAdd => 0xffab,
        K::KpEnter => 0xff8d,
        K::KpEqual => 0xffbd,
        K::LeftShift => 0xffe1,
        K::LeftControl => 0xffe2,
        K::LeftAlt => 0xffe9,
        K::LeftSuper => 0xffeb,
        K::RightShift => 0xffe1,
        K::RightControl => 0xffe2,
        K::RightAlt => 0xffea,
        K::RightSuper => 0xffec,
        K::Menu => 0xff67,
        K::Unknown => 0xffffff,
        _ => 0xffffff,
    }
}

/// GLFW mods -> X11 KeyMask bits (translate_meta expects these).
/// Cmd/Super additionally reports ControlMask so Ctrl-style shortcuts
/// (paste = Ctrl+V) work on macOS, mirroring upstream window_glfw.cpp.
fn mods_to_x11(mods: glfw::Modifiers) -> u32 {
    let mut m = 0u32;
    if mods.contains(glfw::Modifiers::Shift) { m |= 0x1; }
    if mods.contains(glfw::Modifiers::Control) { m |= 0x4; }
    if mods.contains(glfw::Modifiers::Alt) { m |= 0x8; }
    if mods.contains(glfw::Modifiers::Super) { m |= 0x40 | 0x4; }
    m
}

unsafe fn dispatch_event(event: WindowEventT) {
    let Some(win) = STATE.current_window.as_mut() else { return };
    match event {
        WindowEventT::FramebufferSize(w, h) => {
            if w > 0 && h > 0 {
                win.width = w;
                win.height = h;
                if let Some(cb) = win.reshape_cb {
                    cb(w, h);
                }
            }
        }
        WindowEventT::CursorPos(x, y) => {
            if STATE.relative_movement_enabled {
                let dx = x - STATE.relative_movement_last_x as f64;
                let dy = y - STATE.relative_movement_last_y as f64;
                STATE.relative_movement_last_x = x as i32;
                STATE.relative_movement_last_y = y as i32;
                if let Some(cb) = win.mouse_raw_cb {
                    cb(dx, dy);
                }
            } else if let Some(cb) = win.mouse_cb {
                cb(x as i32, y as i32);
            }
        }
        WindowEventT::MouseButton(btn, action, _mods) => {
            // X11 button numbering: 1=left 2=middle 3=right
            let button = match btn {
                glfw::MouseButton::Left => 1,
                glfw::MouseButton::Middle => 2,
                glfw::MouseButton::Right => 3,
                _ => return,
            };
            let (x, y) = cursor_pos();
            if let Some(cb) = win.mouse_button_cb {
                cb(
                    x,
                    y,
                    button,
                    if action == glfw::Action::Press { EGLUT_MOUSE_PRESS } else { EGLUT_MOUSE_RELEASE },
                );
            }
        }
        WindowEventT::Scroll(_x, y) => {
            // wheel -> X11 buttons 4 (up) / 5 (down); press+release pairs
            let (x, y_pos) = cursor_pos();
            let button = if y > 0.0 { 4 } else { 5 };
            if y != 0.0 {
                if let Some(cb) = win.mouse_button_cb {
                    cb(x, y_pos, button, EGLUT_MOUSE_PRESS);
                    cb(x, y_pos, button, EGLUT_MOUSE_RELEASE);
                }
            }
        }
        WindowEventT::Key(key, _scancode, action, mods) => {
            let action_code = match action {
                glfw::Action::Press => 0,
                glfw::Action::Repeat => 2,
                glfw::Action::Release => 1,
            };
            if let Some(cb) = win.special_cb {
                cb(key_to_keysym(key), action_code, mods_to_x11(mods));
            }
        }
        WindowEventT::Char(ch) => {
            if let Some(cb) = win.keyboard_cb {
                let mut s = ch.to_string();
                s.push('\0');
                cb(s.as_ptr() as *mut c_char, 0);
            }
        }
        WindowEventT::Focus(focused) => {
            if let Some(cb) = win.focus_cb {
                cb(if focused { FOCUSED } else { NOT_FOCUSED });
            }
        }        WindowEventT::Close => {
            if let Some(cb) = win.close_cb {
                cb();
            }
        }
        _ => {}
    }
}

fn cursor_pos() -> (i32, i32) {
    let lock = GLFW_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut pos = (0i32, 0i32);
    if let Some(st) = state() {
        let (x, y) = st.win.get_cursor_pos();
        pos = (x as i32, y as i32);
    }
    drop(lock);
    pos
}

/// GLFW poll + dispatch into the registered callbacks. Main thread only.
pub fn pump_events() {
    let lock = match locked() { Some(l) => l, None => return };
    let st = state().unwrap();
    st.glfw.poll_events();
    while let Some((_t, event)) = st.events.receive() {
        unsafe { dispatch_event(event) };
    }
    drop(lock);
}

// ============================================================
// Window creation + GL setup
// ============================================================

#[no_mangle]
pub unsafe extern "C" fn mc_create_window_and_setup_graphics() {
    log::info!("LAUNCHER: Creating window via GLFW (macOS)");
    let tok = create_window("Minecraft");
    if tok.is_null() {
        log::error!("LAUNCHER: window creation failed");
        return;
    }

    fake_egl_set_proc_addr_function(real_get_proc_address());
    fake_egl_install_library();
    fake_egl_setup_gl_overrides();
    fake_egl_save_current_window_handle();
    fake_egl_save_native_window(tok as u64);
    fake_egl_release_context();
    log::info!("LAUNCHER: FakeEGL installed");

    crate::startup::mc_relocate_glesv2_symbols(Some(fake_egl_get_proc_address));
    log::info!("LAUNCHER: Graphics setup complete");
}

extern "C" {
    fn fake_egl_set_proc_addr_function(fn_ptr: *mut c_void);
    fn fake_egl_setup_gl_overrides();
    fn fake_egl_save_current_window_handle();
    fn fake_egl_save_native_window(window: u64);
    fn fake_egl_release_context();
}

/// GL proc resolver for FakeEGL: desktop-GL entry points from NSOpenGLContext.
unsafe extern "C" fn gl_proc_resolver(name: *const c_char) -> *mut c_void {
    if name.is_null() || STATE_PTR.load(Ordering::Acquire).is_null() {
        return std::ptr::null_mut();
    }
    let name_str = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let _lock = GLFW_LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let Some(st) = state() else { return std::ptr::null_mut() };
    st.win.get_proc_address(name_str).map_or(std::ptr::null_mut(), |p| p as usize as *mut c_void)
}

unsafe fn real_get_proc_address() -> *mut c_void {
    gl_proc_resolver as *mut c_void
}

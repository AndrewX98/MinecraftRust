use std::ffi::{c_char, c_void, CString, CStr};
use x11::xlib::*;

use crate::eglut::egl::*;
use crate::eglut::state::*;
use crate::eglut::xinput::*;

pub(crate) unsafe fn eglut_init_inner(dpy_ptr: *mut *mut Display) {
    eprintln!("eglut_init_inner: starting...");
    setlocale(0x6, c"".as_ptr());
    eprintln!("eglut_init_inner: setlocale done");
    STATE.init_time = now_ms();
    let dpy_name = std::ptr::null();
    eprintln!("eglut_init_inner: opening display...");
    let dpy = XOpenDisplay(dpy_name);
    eprintln!("eglut_init_inner: XOpenDisplay returned {:p}", dpy);
    if dpy.is_null() {
        eglutFatalError(b"eglutInit: XOpenDisplay failed\n");
        return;
    }
    *dpy_ptr = dpy;
    STATE.display = dpy;
    let screen = XDefaultScreen(dpy);
    let root = XRootWindow(dpy, screen);
    eprintln!("eglut_init_inner: eglGetDisplay...");
    let egl_dpy = eglGetDisplay(dpy as *mut Display);
    eprintln!("eglut_init_inner: eglGetDisplay returned {:p}", egl_dpy);
    if egl_dpy.is_null() {
        eglutFatalError(b"eglutInit: eglGetDisplay failed\n");
        return;
    }
    STATE.egl_dpy = egl_dpy;
    let mut ver_major: EGLint = 0;
    let mut ver_minor: EGLint = 0;
    eprintln!("eglut_init_inner: eglInitialize...");
    let ok = eglInitialize(egl_dpy, &mut ver_major, &mut ver_minor);
    eprintln!("eglut_init_inner: eglInitialize returned {}", ok);
    if ok == EGL_FALSE {
        eglutFatalError(b"eglutInit: eglInitialize failed\n");
        return;
    }
    if STATE.verbose {
        let s = eglQueryString(egl_dpy, 0x3029);
        if !s.is_null() {
            let st = CStr::from_ptr(s).to_string_lossy().into_owned();
            println!("libegl vendor: {:?}", st);
        }
        let s = eglQueryString(egl_dpy, 0x302A);
        if !s.is_null() {
            let st = CStr::from_ptr(s).to_string_lossy().into_owned();
            println!("libegl version: {:?}", st);
        }
    }
    println!("eglutInit: dpy={:?} screen={} root={:x} egl_dpy={:?} ver={}.{}",
             dpy, screen, root, egl_dpy, ver_major, ver_minor);

    eprintln!("eglut_init_inner: initializing atoms...");
    STATE.xdnd_drop = XInternAtom(dpy, "XdndDrop\0".as_ptr() as *const c_char, 0);
    STATE.xdnd_type_list = XInternAtom(dpy, "XdndTypeList\0".as_ptr() as *const c_char, 0);
    STATE.xdnd_selection = XInternAtom(dpy, "XdndSelection\0".as_ptr() as *const c_char, 0);
    STATE.xdnd_enter = XInternAtom(dpy, "XdndEnter\0".as_ptr() as *const c_char, 0);
    STATE.xdnd_position = XInternAtom(dpy, "XdndPosition\0".as_ptr() as *const c_char, 0);
    STATE.xdnd_status = XInternAtom(dpy, "XdndStatus\0".as_ptr() as *const c_char, 0);
    STATE.xdnd_leave = XInternAtom(dpy, "XdndLeave\0".as_ptr() as *const c_char, 0);
    STATE.xdnd_finished = XInternAtom(dpy, "XdndFinished\0".as_ptr() as *const c_char, 0);
    STATE.xdnd_action_copy = XInternAtom(dpy, "XdndActionCopy\0".as_ptr() as *const c_char, 0);
    STATE.xtext_uri_list = XInternAtom(dpy, "text/uri-list\0".as_ptr() as *const c_char, 0);
    eprintln!("eglut_init_inner: atoms done");

    XINPUT_RT = Some(XInputRuntime {
        lib_xi2: None, xi2_available: false, xi_opcode: 0,
        xisuppress: None, xiselect_events: None, xiquery_device: None,
        xifree_device: None, xiquery_extension: None, xiget_property: None,
        xiseti_focus: None, xigeti_focus: None,
    });
}

pub(crate) unsafe fn eglutChooseConfig(api_mask: i32, attribs_surface_type: EGLint) -> EGLConfig {
    let dpy = STATE.egl_dpy;
    let mut renderable_type = 0;
    let mut api = EGL_OPENGL_ES_API;
    if (api_mask & EGLUT_OPENGL_BIT) != 0 {
        renderable_type = EGL_OPENGL_BIT;
        api = EGL_OPENGL_API;
    } else if (api_mask & EGLUT_OPENGL_ES2_BIT) != 0 {
        renderable_type = EGL_OPENGL_ES2_BIT;
    } else if (api_mask & EGLUT_OPENGL_ES1_BIT) != 0 {
        renderable_type = EGL_OPENGL_ES_BIT;
    }
    if (api_mask & EGLUT_OPENVG_BIT) != 0 {
        renderable_type = EGL_OPENVG_BIT;
        api = EGL_OPENVG_API;
    }
    eglBindAPI(api);
    let attribs: [EGLint; 15] = [
        EGL_SURFACE_TYPE, attribs_surface_type,
        EGL_RED_SIZE, 8, EGL_GREEN_SIZE, 8, EGL_BLUE_SIZE, 8, EGL_ALPHA_SIZE, 8,
        EGL_DEPTH_SIZE, 24,
        EGL_RENDERABLE_TYPE, renderable_type,
        EGL_NONE,
    ];
    let mut config: EGLConfig = std::ptr::null_mut();
    let mut num_config: EGLint = 0;
    eglChooseConfig(dpy, attribs.as_ptr(), &mut config, 1, &mut num_config);
    if num_config == 0 {
        println!("eglutInit: no matching EGL config found");
        eglChooseConfig(dpy, attribs.as_ptr(), std::ptr::null_mut(), 0, &mut num_config);
        println!("  ... total matching configs: {}", num_config);
    }
    let mut vid: EGLint = 0;
    eglGetConfigAttrib(dpy, config, EGL_NATIVE_VISUAL_ID, &mut vid);
    if vid != 0 {
        XVisualInfo { visual: std::ptr::null_mut(), visualid: vid as u64, ..std::mem::zeroed() };
    } else {
        println!("eglutInit: EGLConfig has no native visual (non-native config)");
    }
    config
}

#[no_mangle]
pub unsafe extern "C" fn eglutNativeInitWindow(dpy: *mut Display, xwin: Window, width: i32, height: i32) {
    STATE.display = dpy;
    let egl_dpy = eglGetDisplay(dpy);
    if egl_dpy.is_null() { return; }
    STATE.egl_dpy = egl_dpy;
    let mut ver_major = 0; let mut ver_minor = 0;
    eglInitialize(egl_dpy, &mut ver_major, &mut ver_minor);
    STATE.api_mask = EGLUT_OPENGL_ES2_BIT;
    let config = eglutChooseConfig(STATE.api_mask, EGL_WINDOW_BIT);
    let context = eglCreateContext(egl_dpy, config, EGL_NO_CONTEXT, [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE].as_ptr());
    let surface = eglCreateWindowSurface(egl_dpy, config, xwin as EGLNativeWindowType, [EGL_NONE].as_ptr());
    STATE.current_window = Some(Box::new(EglutWindow {
        xwin, width, height, x: 0, y: 0,
        context, surface, config,
        index: 0,
        reshape_cb: None, display_cb: None, keyboard_cb: None, drop_cb: None,
        special_cb: None, paste_cb: None, mouse_cb: None, mouse_raw_cb: None,
        mouse_button_cb: None, touch_start_cb: None, touch_update_cb: None,
        touch_end_cb: None, focus_cb: None, close_cb: None, keyboardstate: 0,
    }));
    STATE.current_xwin = xwin;
    STATE.num_windows = 1;
    eglMakeCurrent(egl_dpy, surface, surface, context);
    eglSwapInterval(egl_dpy, 1);
}

#[no_mangle]
pub unsafe extern "C" fn eglutDestroyWindow() {
    if let Some(win) = STATE.current_window.take() {
        let dpy = STATE.egl_dpy;
        eglMakeCurrent(dpy, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
        eglDestroySurface(dpy, win.surface);
        eglDestroyContext(dpy, win.context);
        XDestroyWindow(STATE.display, win.xwin);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutFini() {
    eglutDestroyWindow();
    let dpy = STATE.egl_dpy;
    eglTerminate(dpy);
    if !STATE.display.is_null() {
        XCloseDisplay(STATE.display);
    }
    STATE.display = std::ptr::null_mut();
}

#[no_mangle]
pub unsafe extern "C" fn eglutShowWindow() {
    if let Some(win) = &STATE.current_window {
        XMapRaised(STATE.display, win.xwin);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutMakeCurrent() {
    if let Some(win) = &STATE.current_window {
        eglMakeCurrent(STATE.egl_dpy, win.surface, win.surface, win.context);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowWidth() -> i32 {
    STATE.current_window.as_ref().map(|w| w.width).unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowHeight() -> i32 {
    STATE.current_window.as_ref().map(|w| w.height).unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowX() -> i32 {
    STATE.current_window.as_ref().map(|w| w.x).unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowY() -> i32 {
    STATE.current_window.as_ref().map(|w| w.y).unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowSize(w: *mut i32, h: *mut i32) {
    if let Some(win) = &STATE.current_window {
        *w = win.width; *h = win.height;
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowPos(x: *mut i32, y: *mut i32) {
    if let Some(win) = &STATE.current_window {
        *x = win.x; *y = win.y;
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutPostRedisplay() {
    STATE.redisplay = true;
}

#[no_mangle]
pub unsafe extern "C" fn eglutSwapBuffers() {
    let dpy = STATE.egl_dpy;
    let surface = STATE.current_window.as_ref().map(|w| w.surface).unwrap_or(std::ptr::null_mut());
    if !surface.is_null() {
        eglSwapBuffers(dpy, surface);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutSwapInterval(interval: i32) {
    eglSwapInterval(STATE.egl_dpy, interval);
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindow() -> u64 {
    STATE.current_xwin
}

#[no_mangle]
pub unsafe extern "C" fn eglutSetWindowTitle(title: *const c_char) {
    if let Some(win) = &STATE.current_window {
        let s = CStr::from_ptr(title).to_string_lossy();
        XStoreName(STATE.display, win.xwin, CString::new(s.as_ref()).unwrap().as_ptr());
        let wm_hints = XAllocWMHints();
        if !wm_hints.is_null() {
            (*wm_hints).flags = 1 << 1 | 1 << 0;
            (*wm_hints).input = 1;
            (*wm_hints).initial_state = 1;
            XSetWMHints(STATE.display, win.xwin, wm_hints);
            XFree(wm_hints as *mut c_void);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetDisplay() -> *mut Display {
    STATE.display
}

unsafe fn eglutFatalError(msg: &[u8]) {
    eglutError(msg.as_ptr() as *const c_char);
}

#[no_mangle]
pub unsafe extern "C" fn eglutError(msg: *const c_char) {
    let s = CStr::from_ptr(msg).to_string_lossy();
    eprint!("{}", s);
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowContext() -> EGLContext {
    STATE.current_window.as_ref().map(|w| w.context).unwrap_or(std::ptr::null_mut())
}

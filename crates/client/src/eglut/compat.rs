use std::io::Write;
use std::ffi::{c_char, c_void, CStr, CString};
use x11::xlib::*;

use crate::eglut::egl::*;
use crate::eglut::state::*;
use crate::eglut::window::{eglutChooseConfig, eglut_init_inner};

static mut INIT_WIN_W: i32 = 300;
static mut INIT_WIN_H: i32 = 300;
static mut INIT_CLASS_INSTANCE: Option<CString> = None;
static mut INIT_CLASS_NAME: Option<CString> = None;

#[no_mangle]
pub unsafe extern "C" fn eglutInitWindowSize(width: i32, height: i32) {
    INIT_WIN_W = width;
    INIT_WIN_H = height;
}

#[no_mangle]
pub unsafe extern "C" fn eglutInitAPIMask(mask: i32) {
    STATE.api_mask = mask;
}

#[no_mangle]
pub unsafe extern "C" fn eglutInitX11ClassInstanceName(value: *const c_char) {
    if value.is_null() {
        INIT_CLASS_INSTANCE = None;
    } else {
        INIT_CLASS_INSTANCE = Some(CStr::from_ptr(value).to_owned());
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutInitX11ClassName(value: *const c_char) {
    if value.is_null() {
        INIT_CLASS_NAME = None;
    } else {
        INIT_CLASS_NAME = Some(CStr::from_ptr(value).to_owned());
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutInit(argc: i32, argv: *mut *mut c_char) {
    let mut stderr = std::io::stderr().lock();
    let _ = writeln!(&mut stderr, "eglutInit(Rust): ENTER argc={}", argc);
    if STATE.display.is_null() {
        let _ = writeln!(&mut stderr, "eglutInit(Rust): calling eglut_init_inner...");
        let mut dpy: *mut Display = std::ptr::null_mut();
        eglut_init_inner(&mut dpy);
        let _ = writeln!(&mut stderr, "eglutInit(Rust): init_inner done display={:p}", dpy);

    } else {
        let _ = writeln!(&mut stderr, "eglutInit(Rust): display already initialized");
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutCreateWindow(title: *const c_char) -> i32 {
    if STATE.display.is_null() {
        let mut dpy: *mut Display = std::ptr::null_mut();
        eglut_init_inner(&mut dpy);
    }
    let dpy = STATE.display;
    let screen = XDefaultScreen(dpy);
    let root = XRootWindow(dpy, screen);
    let width = INIT_WIN_W;
    let height = INIT_WIN_H;
    let egl_dpy = STATE.egl_dpy;
    let config = eglutChooseConfig(STATE.api_mask, EGL_WINDOW_BIT);

    // Create X11 window with visual matching the EGL config
    let mut vid: EGLint = 0;
    eglGetConfigAttrib(egl_dpy, config, EGL_NATIVE_VISUAL_ID, &mut vid);
    let (xwin, _cmap) = if vid != 0 {
        let mut vis_template: XVisualInfo = std::mem::zeroed();
        vis_template.visualid = vid as u64;
        let mut num_visuals: i32 = 0;
        let vis_info = XGetVisualInfo(dpy, VisualIDMask, &mut vis_template, &mut num_visuals);
        if !vis_info.is_null() {
            let mut attr: XSetWindowAttributes = std::mem::zeroed();
            attr.background_pixel = 0;
            attr.border_pixel = 0;
            attr.colormap = XCreateColormap(dpy, root, (*vis_info).visual, 0);
            attr.event_mask = ExposureMask | KeyPressMask | KeyReleaseMask
                | ButtonPressMask | ButtonReleaseMask | PointerMotionMask
                | FocusChangeMask | StructureNotifyMask;
            let mask = CWBackPixel | CWBorderPixel | CWColormap | CWEventMask;
            let w = XCreateWindow(dpy, root, 0, 0, width as u32, height as u32,
                                  0, (*vis_info).depth, InputOutput as u32, (*vis_info).visual,
                                  mask, &mut attr);
            XFree(vis_info as *mut c_void);
            (w, attr.colormap)
        } else {
            let w = XCreateSimpleWindow(dpy, root, 0, 0, width as u32, height as u32, 0, 0, 0);
            let event_masks = ExposureMask | KeyPressMask | KeyReleaseMask
                | ButtonPressMask | ButtonReleaseMask | PointerMotionMask
                | FocusChangeMask | StructureNotifyMask;
            XSelectInput(dpy, w, event_masks);
            (w, 0)
        }
    } else {
        let w = XCreateSimpleWindow(dpy, root, 0, 0, width as u32, height as u32, 0, 0, 0);
        let event_masks = ExposureMask | KeyPressMask | KeyReleaseMask
            | ButtonPressMask | ButtonReleaseMask | PointerMotionMask
            | FocusChangeMask | StructureNotifyMask;
        XSelectInput(dpy, w, event_masks);
        (w, 0)
    };

    let title_cstr = CString::new(CStr::from_ptr(title).to_string_lossy().as_ref()).unwrap();
    XStoreName(dpy, xwin, title_cstr.as_ptr());
    let mut wm_delete = XInternAtom(dpy, "WM_DELETE_WINDOW\0".as_ptr() as *const c_char, 0);
    XSetWMProtocols(dpy, xwin, &mut wm_delete, 1);
    XMapWindow(dpy, xwin);
    XFlush(dpy);
    // Wait briefly for MapNotify so the first game-thread eglCreateWindowSurface
    // sees a mapped window (some Mesa paths present black until mapped).
    {
        let mut mapped = false;
        for _ in 0..50 {
            while XPending(dpy) != 0 {
                let mut ev: XEvent = std::mem::zeroed();
                XNextEvent(dpy, &mut ev);
                if ev.get_type() == MapNotify {
                    mapped = true;
                }
            }
            if mapped {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        if !mapped {
            eprintln!("eglutCreateWindow: MapNotify not seen yet (continuing)");
        }
    }

    // NOTE: EGL context and surface are NOT created here. Mesa's X11 EGL
    // backend has thread affinity — a context/surface created on one thread
    // cannot be made current on another thread (EGL_BAD_ACCESS). Since the
    // game uses a separate render thread, we defer real EGL object creation
    // to fake_egl_make_current on the game/render thread itself.

    let win_idx = STATE.num_windows;
    STATE.num_windows += 1;
    STATE.current_window = Some(Box::new(EglutWindow {
        xwin, width, height, x: 0, y: 0,
        context: std::ptr::null_mut(),
        surface: std::ptr::null_mut(),
        config,
        index: win_idx,
        reshape_cb: None, display_cb: None, keyboard_cb: None, drop_cb: None,
        special_cb: None, paste_cb: None, mouse_cb: None, mouse_raw_cb: None,
        mouse_button_cb: None, touch_start_cb: None, touch_update_cb: None,
        touch_end_cb: None, focus_cb: None, close_cb: None, keyboardstate: 0,
    }));

    STATE.current_xwin = xwin;
    if !title.is_null() {
        let s = CStr::from_ptr(title).to_string_lossy();
        eprintln!("eglutCreateWindow: '{}' -> xwin={:x} idx={} egl_dpy={:p}",
                  s, xwin, win_idx, egl_dpy);
    }
    STATE.current_xwin = xwin;
    win_idx as i32
}

#[no_mangle]
pub unsafe extern "C" fn eglutScreenWidth() -> i32 {
    let dpy = STATE.display;
    if dpy.is_null() { return 1920; }
    let screen = XDefaultScreen(dpy);
    XDisplayWidth(dpy, screen)
}

#[no_mangle]
pub unsafe extern "C" fn eglutScreenHeight() -> i32 {
    let dpy = STATE.display;
    if dpy.is_null() { return 1080; }
    let screen = XDefaultScreen(dpy);
    XDisplayHeight(dpy, screen)
}

#[no_mangle]
pub unsafe extern "C" fn eglutSetWindowIcon(path: *const c_char) {
    if !path.is_null() {
        let s = CStr::from_ptr(path).to_string_lossy();
        println!("eglutSetWindowIcon: {}", s);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutGet(param: i32) -> i32 {
    match param {
        0 => { // EGLUT_ELAPSED_TIME
            let now = now_ms();
            (now - STATE.init_time) as i32
        }
        1 => { // EGLUT_FULLSCREEN_MODE
            STATE.window_fullscreen
        }
        _ => 0,
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutToggleFullscreen() {
    STATE.window_fullscreen = if STATE.window_fullscreen == FULLSCREEN { WINDOWED } else { FULLSCREEN };
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowHandle() -> u64 {
    STATE.current_xwin
}

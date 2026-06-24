use std::ffi::{c_char, c_uint, c_void, CStr};
use x11::xlib::*;

use crate::eglut::state::*;

#[no_mangle]
pub unsafe extern "C" fn eglutSetMousePointerLocked(locked: i32) {
    CURSOR_GRABBED = locked != 0;
    RELATIVE_MOVEMENT_ENABLED = locked != 0;
    RELATIVE_MOVEMENT_RAW_MODE = false;
    if locked != 0 {
        if let Some(win) = &STATE.current_window {
            RELATIVE_MOVEMENT_LAST_X = win.width / 2;
            RELATIVE_MOVEMENT_LAST_Y = win.height / 2;
            XWarpPointer(STATE.display, 0, win.xwin, 0, 0, 0, 0, RELATIVE_MOVEMENT_LAST_X, RELATIVE_MOVEMENT_LAST_Y);
        }
    }
    if let Some(win) = &STATE.current_window {
        let dpy = STATE.display;
        if dpy.is_null() { return; }
        if locked != 0 {
            XGrabPointer(dpy, win.xwin, 0,
                         (PointerMotionMask | ButtonPressMask | ButtonReleaseMask) as u32,
                         GrabModeAsync, GrabModeAsync,
                         0, 0, CurrentTime);
        } else {
            XUngrabPointer(dpy, CurrentTime);
        }
    }
    eglutSetMousePointerVisibility(if locked != 0 { 0 } else { 1 });
}

#[no_mangle]
pub unsafe extern "C" fn eglutSetMousePointerVisibility(visible: i32) {
    if let Some(win) = &STATE.current_window {
        let dpy = STATE.display;
        if dpy.is_null() { return; }
        if visible == 0 {
            let empty_data: [u8; 8] = [0; 8];
            let empty_bitmap = XCreateBitmapFromData(dpy, win.xwin, empty_data.as_ptr() as *const c_char, 8, 8);
            let mut black = XColor { pixel: 0, red: 0, green: 0, blue: 0, flags: 0, pad: 0 };
            let cursor = XCreatePixmapCursor(dpy, empty_bitmap, empty_bitmap, &mut black, &mut black, 0, 0);
            XDefineCursor(dpy, win.xwin, cursor);
            XFreeCursor(dpy, cursor);
            XFreePixmap(dpy, empty_bitmap);
        } else {
            XUndefineCursor(dpy, win.xwin);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutSetCursor(cursor: i32) {
    if let Some(win) = &STATE.current_window {
        let dpy = STATE.display;
        if dpy.is_null() { return; }
        let cm = XCreateFontCursor(dpy, cursor as c_uint);
        XDefineCursor(dpy, win.xwin, cm);
        XFreeCursor(dpy, cm);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetMousePointerLocked() -> i32 {
    if CURSOR_GRABBED { POINTER_LOCKED } else { POINTER_UNLOCKED }
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetMousePointerVisibility() -> i32 {
    POINTER_VISIBLE
}

#[no_mangle]
pub unsafe extern "C" fn eglutSetClipboardText(text: *const c_char) {
    eglutCopyClipboard(text);
}

#[no_mangle]
pub unsafe extern "C" fn eglutCopyClipboard(text: *const c_char) {
    if text.is_null() { return; }
    CLIPBOARD_TEXT = Some(CStr::from_ptr(text).to_owned());
    let dpy = STATE.display;
    if dpy.is_null() { return; }
    let clipboard_atom = XInternAtom(dpy, "CLIPBOARD\0".as_ptr() as *const c_char, 0);
    if let Some(win) = &STATE.current_window {
        XSetSelectionOwner(dpy, clipboard_atom, win.xwin, CurrentTime);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutRequestPaste() {
    let dpy = STATE.display;
    if dpy.is_null() { return; }
    let clipboard_atom = XInternAtom(dpy, "CLIPBOARD\0".as_ptr() as *const c_char, 0);
    let utf8_string = XInternAtom(dpy, "UTF8_STRING\0".as_ptr() as *const c_char, 0);
    if let Some(win) = &STATE.current_window {
        XConvertSelection(dpy, clipboard_atom, utf8_string, clipboard_atom, win.xwin, CurrentTime);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutRequestSelection() {
    let dpy = STATE.display;
    if dpy.is_null() { return; }
    let primary_atom = XInternAtom(dpy, "PRIMARY\0".as_ptr() as *const c_char, 0);
    let utf8_string = XInternAtom(dpy, "UTF8_STRING\0".as_ptr() as *const c_char, 0);
    if let Some(win) = &STATE.current_window {
        XConvertSelection(dpy, primary_atom, utf8_string, primary_atom, win.xwin, CurrentTime);
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutFullscreen() {
    STATE.window_fullscreen = FULLSCREEN;
}

#[no_mangle]
pub unsafe extern "C" fn eglutWindowed() {
    STATE.window_fullscreen = WINDOWED;
}

#[no_mangle]
pub unsafe extern "C" fn eglutGetWindowedSize(w: *mut i32, h: *mut i32) {
    if let Some(win) = &STATE.current_window {
        *w = win.width; *h = win.height;
    }
}

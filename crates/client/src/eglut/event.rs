use std::ffi::{c_char, c_int, c_long, c_ulong, c_void, CStr, CString};
use x11::xlib::*;

use crate::eglut::mouse::{eglutSetMousePointerLocked, eglutSetMousePointerVisibility};
use crate::eglut::state::*;
use crate::eglut::util::*;
use crate::eglut::xinput::*;

pub unsafe fn handle_xinput_event(_xevent: *mut XEvent) -> bool {
    if XINPUT_RT.as_ref().map(|r| !r.xi2_available).unwrap_or(true) { return false; }
    false
}

unsafe fn handle_xdnd_enter(dpy: *mut Display, ev: &XClientMessageEvent) {
    STATE.dnd_source = ev.data.get_long(0);
    let more_types = (ev.data.get_long(1) & 1) != 0;
    STATE.dnd_version = ev.data.get_long(1) >> 24;
    if more_types {
        let mut actual_type: Atom = 0; let mut actual_format: c_int = 0;
        let mut nitems: u64 = 0; let mut leftover: u64 = 0;
        let mut prop: *mut u8 = std::ptr::null_mut();
        XGetWindowProperty(dpy, STATE.dnd_source as u64, STATE.xdnd_type_list,
                           0, 0x8000000, 0, 4 as Atom,
                           &mut actual_type, &mut actual_format, &mut nitems, &mut leftover, &mut prop);
        if !prop.is_null() { XFree(prop as *mut c_void); }
        STATE.dnd_format = actual_format;
    } else {
        STATE.dnd_format = ev.data.get_long(2) as i32;
    }
}

unsafe fn handle_xdnd_position(dpy: *mut Display, ev: &XClientMessageEvent) -> bool {
    let xev = XEvent::from(XClientMessageEvent {
        type_: ClientMessage as c_int, serial: ev.serial, send_event: 0,
        display: dpy, window: ev.window,
        message_type: STATE.xdnd_status, format: 32,
        data: ClientMessageData::from([ev.window as c_long, 0, 0, 0, STATE.xdnd_action_copy as c_long]),
    });
    XSendEvent(dpy, STATE.dnd_source as u64, 0, 0xFFu32 as c_long, &xev as *const XEvent as *mut XEvent);
    XFlush(dpy);
    true
}

unsafe fn handle_xdnd_drop(dpy: *mut Display, ev: &XClientMessageEvent) {
    if STATE.dnd_format == STATE.xtext_uri_list as i32 {
        XConvertSelection(dpy, STATE.xdnd_selection, STATE.xtext_uri_list, STATE.xdnd_selection,
                          ev.window, ev.data.get_long(2) as u64);
        XFlush(dpy);
    }
    let xev = XEvent::from(XClientMessageEvent {
        type_: ClientMessage as c_int, serial: 0, send_event: 0,
        display: dpy, window: ev.window,
        message_type: STATE.xdnd_finished, format: 32,
        data: ClientMessageData::from([ev.window as c_long, 1, STATE.xdnd_action_copy as c_long, 0, 0]),
    });
    XSendEvent(dpy, STATE.dnd_source as u64, 0, 0xFFu32 as c_long, &xev as *const XEvent as *mut XEvent);
    XFlush(dpy);
}

unsafe fn handle_selection_notify(dpy: *mut Display, ev: &XSelectionEvent) {
    if ev.property == 0 { return; }
    let mut actual_type: Atom = 0; let mut actual_format: c_int = 0;
    let mut nitems: u64 = 0; let mut leftover: u64 = 0;
    let mut prop: *mut u8 = std::ptr::null_mut();
    XGetWindowProperty(dpy, ev.requestor, ev.property, 0, 0x8000000,
                       0, 0, &mut actual_type, &mut actual_format,
                       &mut nitems, &mut leftover, &mut prop);
    if !prop.is_null() && nitems > 0 {
        let data = std::slice::from_raw_parts(prop, nitems as usize);
        let s = std::str::from_utf8(data).unwrap_or("");
        for line in s.lines() {
            let line = line.trim();
            if let Some(stripped) = line.strip_prefix("file://") {
                let decoded_len = url_decode_len(stripped.as_ptr() as *const c_char);
                let decoded_len = decoded_len + 1;
                let mut decoded_buf = Vec::with_capacity(decoded_len as usize);
                let decoded = decoded_buf.as_mut_ptr();
                url_decode(stripped.as_ptr() as *const c_char, decoded);
                let decoded_str = CStr::from_ptr(decoded).to_string_lossy().into_owned();
                let cstr = CString::new(decoded_str).unwrap();
                if let Some(win) = &STATE.current_window {
                    if let Some(cb) = win.drop_cb {
                        cb(cstr.as_ptr());
                    }
                }
            }
        }
    }
    if !prop.is_null() { XFree(prop as *mut c_void); }
    XDeleteProperty(dpy, ev.requestor, ev.property);
}

unsafe fn handle_selection_request(dpy: *mut Display, ev: &XSelectionRequestEvent) {
    let property = if ev.target == XInternAtom(dpy, "UTF8_STRING\0".as_ptr() as *const c_char, 0)
                      || ev.target == XInternAtom(dpy, "text/plain\0".as_ptr() as *const c_char, 0)
                      || ev.target == XInternAtom(dpy, "STRING\0".as_ptr() as *const c_char, 0)
                      || ev.target == XInternAtom(dpy, "TEXT\0".as_ptr() as *const c_char, 0)
                      || ev.target == XInternAtom(dpy, "text/plain;charset=utf-8\0".as_ptr() as *const c_char, 0)
                      || ev.target == XInternAtom(dpy, "text/plain;charset=UTF-8\0".as_ptr() as *const c_char, 0)
                      || ev.target == XInternAtom(dpy, "TARGETS\0".as_ptr() as *const c_char, 0)
    {
        let dummy_text = CString::new("").unwrap().into_raw();
        if !CLIPBOARD_TEXT.as_ref().map(|c| c.as_bytes().is_empty()).unwrap_or(true) {
            let ct = CLIPBOARD_TEXT.as_ref().unwrap().as_ptr();
            XChangeProperty(dpy, ev.requestor, ev.property, ev.target, 8, 0, ct as *const u8,
                            CLIPBOARD_TEXT.as_ref().unwrap().as_bytes().len() as i32);
        } else {
            XChangeProperty(dpy, ev.requestor, ev.property, ev.target, 8, 0,
                            dummy_text as *const u8, 0);
        }
        ev.property
    } else {
        0
    };
    let xev = XEvent { selection_clear: XSelectionClearEvent {
        type_: SelectionNotify as c_int, serial: 0, send_event: 0,
        display: dpy, window: ev.requestor,
        selection: ev.selection, time: ev.time,
    }};
    XSendEvent(dpy, ev.requestor, 0, 0, &xev as *const XEvent as *mut XEvent);
    XFlush(dpy);
}

pub unsafe fn handle_xevent(dpy: *mut Display, xevent: &mut XEvent) {
    match xevent.get_type() {
        Expose => {
            STATE.redisplay = true;
        }
        ConfigureNotify => {
            let ev: &XConfigureEvent = xevent.as_ref();
            if let Some(win) = &mut STATE.current_window {
                win.width = ev.width;
                win.height = ev.height;
                if let Some(cb) = win.reshape_cb {
                    cb(ev.width, ev.height);
                }
                STATE.redisplay = true;
            }
        }
        FocusIn => {
            if RELATIVE_MOVEMENT_ENABLED {
                eglutSetMousePointerLocked(POINTER_LOCKED);
            }
            if let Some(win) = &STATE.current_window {
                if let Some(cb) = win.focus_cb {
                    cb(FOCUSED);
                }
            }
        }
        FocusOut => {
            if RELATIVE_MOVEMENT_ENABLED {
                eglutSetMousePointerVisibility(POINTER_VISIBLE);
            }
            if let Some(win) = &STATE.current_window {
                if let Some(cb) = win.focus_cb {
                    cb(NOT_FOCUSED);
                }
            }
        }
        KeyPress | KeyRelease => {
            let (xwin, keycode, time, ev_state) = {
                let ev: &XKeyEvent = xevent.as_ref();
                (ev.window, ev.keycode, ev.time, ev.state)
            };
            let mut action = if xevent.get_type() == KeyPress { EGLUT_KEY_PRESS } else { EGLUT_KEY_RELEASE };
            if action == EGLUT_KEY_RELEASE && XPending(dpy) != 0 {
                let mut ahead: XEvent = std::mem::zeroed();
                XPeekEvent(dpy, &mut ahead);
                if ahead.get_type() == KeyPress {
                    let ahead_ev: &XKeyEvent = ahead.as_ref();
                    if ahead_ev.window == xwin && ahead_ev.keycode == keycode && ahead_ev.time == time {
                        action = EGLUT_KEY_REPEAT;
                        XNextEvent(dpy, &mut ahead);
                        *xevent = ahead;
                    }
                }
            }
            let ev: &XKeyEvent = xevent.as_ref();
            let mut buf = [0i8; 32];
            let mut keysym: u64 = 0;
            let mut status = 0;
            let r = if !X11_IC.is_null() {
                Xutf8LookupString(X11_IC as XIC, ev as *const XKeyEvent as *mut XKeyPressedEvent,
                                  buf.as_mut_ptr(), 31, &mut keysym, &mut status)
            } else {
                let mut ks: c_ulong = 0;
                XLookupString(ev as *const XKeyEvent as *mut XKeyEvent,
                              buf.as_mut_ptr(), 31, &mut ks, std::ptr::null_mut())
            };
            let eglut_sym = key_sym_to_eglut(XLookupKeysym(ev as *const XKeyEvent as *mut XKeyEvent, 0) as u64);
            if let Some(win) = &STATE.current_window {
                if r > 0 {
                    if let Some(cb) = win.keyboard_cb {
                        cb(buf.as_mut_ptr(), action);
                    }
                }
                if let Some(cb) = win.special_cb {
                    cb(eglut_sym, action, ev_state as u32);
                }
            }
        }
        ButtonPress => {
            let ev: &XButtonEvent = xevent.as_ref();
            if let Some(win) = &STATE.current_window {
                if let Some(cb) = win.mouse_button_cb {
                    cb(ev.x, ev.y, ev.button as i32, EGLUT_MOUSE_PRESS);
                }
            }
        }
        ButtonRelease => {
            let ev: &XButtonEvent = xevent.as_ref();
            if let Some(win) = &STATE.current_window {
                if let Some(cb) = win.mouse_button_cb {
                    cb(ev.x, ev.y, ev.button as i32, EGLUT_MOUSE_RELEASE);
                }
            }
        }
        MotionNotify => {
            let ev: &XMotionEvent = xevent.as_ref();
            if let Some(win) = &STATE.current_window {
                if RELATIVE_MOVEMENT_ENABLED {
                    let center_x = win.width / 2;
                    let center_y = win.height / 2;
                    if let Some(cb) = win.mouse_raw_cb {
                        if !RELATIVE_MOVEMENT_RAW_MODE {
                            cb((ev.x - RELATIVE_MOVEMENT_LAST_X) as f64, (ev.y - RELATIVE_MOVEMENT_LAST_Y) as f64);
                        }
                    }
                    RELATIVE_MOVEMENT_LAST_X = center_x;
                    RELATIVE_MOVEMENT_LAST_Y = center_y;
                    if ev.x != center_x || ev.y != center_y {
                        XWarpPointer(STATE.display, 0, win.xwin, 0, 0, 0, 0, center_x, center_y);
                    }
                } else if let Some(cb) = win.mouse_cb {
                    cb(ev.x, ev.y);
                }
            }
        }
        ClientMessage => {
            handle_client_message(dpy, &xevent.client_message);
        }
        SelectionRequest => {
            handle_selection_request(dpy, &xevent.selection_request);
        }
        SelectionNotify => {
            handle_selection_notify(dpy, &xevent.selection);
        }
        _ => {
            if handle_xinput_event(xevent) {
                if let Some(win) = &STATE.current_window {
                    if let Some(cb) = win.display_cb { if STATE.redisplay { cb(); } }
                }
            }
        }
    }
}

unsafe fn handle_client_message(dpy: *mut Display, ev: &XClientMessageEvent) {
    if ev.message_type == STATE.xdnd_enter {
        handle_xdnd_enter(dpy, ev);
    } else if ev.message_type == STATE.xdnd_position {
        handle_xdnd_position(dpy, ev);
    } else if ev.message_type == STATE.xdnd_drop {
        handle_xdnd_drop(dpy, ev);
    } else if ev.message_type == STATE.xdnd_leave {
    } else if ev.format == 32 && ev.data.get_long(0) as u64 == STATE.xdnd_drop {
    } else {
        let wm_protocols = XInternAtom(dpy, "WM_PROTOCOLS\0".as_ptr() as *const c_char, 0);
        let wm_delete = XInternAtom(dpy, "WM_DELETE_WINDOW\0".as_ptr() as *const c_char, 0);
        if ev.message_type == wm_protocols && ev.data.get_long(0) as u64 == wm_delete {
            if let Some(win) = &STATE.current_window {
                if let Some(cb) = win.close_cb {
                    cb();
                }
            }
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutPollEvents() {
    let dpy = STATE.display;
    if dpy.is_null() { return; }
    XLockDisplay(dpy);
    while XPending(dpy) != 0 {
        let mut xevent: XEvent = std::mem::zeroed();
        XNextEvent(dpy, &mut xevent);
        handle_xevent(dpy, &mut xevent);
    }
    XUnlockDisplay(dpy);
    if STATE.redisplay {
        if let Some(win) = &STATE.current_window {
            if let Some(cb) = win.display_cb {
                cb();
            }
        }
        STATE.redisplay = false;
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutMainLoop() {
    let dpy = STATE.display;
    if dpy.is_null() { return; }
    loop {
        let mut xevent: XEvent = std::mem::zeroed();
        XNextEvent(dpy, &mut xevent);
        handle_xevent(dpy, &mut xevent);
        if let Some(idle) = STATE.idle_cb {
            idle();
        }
        if STATE.redisplay {
            if let Some(win) = &STATE.current_window {
                if let Some(cb) = win.display_cb {
                    cb();
                }
            }
            STATE.redisplay = false;
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn eglutWarpMousePointer(x: i32, y: i32) {
    if let Some(win) = &STATE.current_window {
        XWarpPointer(STATE.display, 0, win.xwin, 0, 0, 0, 0, x as i32, y as i32);
    }
}



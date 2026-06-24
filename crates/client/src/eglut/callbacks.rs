use crate::eglut::state::*;

#[no_mangle]
pub unsafe extern "C" fn eglutDisplayFunc(func: EGLUTdisplayCB) {
    if let Some(win) = &mut STATE.current_window { win.display_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutReshapeFunc(func: EGLUTreshapeCB) {
    if let Some(win) = &mut STATE.current_window { win.reshape_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutKeyboardFunc(func: EGLUTkeyboardCB) {
    if let Some(win) = &mut STATE.current_window { win.keyboard_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutDropFunc(func: EGLUTdropCB) {
    if let Some(win) = &mut STATE.current_window { win.drop_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutSpecialFunc(func: EGLUTspecialCB) {
    if let Some(win) = &mut STATE.current_window { win.special_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutPasteFunc(func: EGLUTpasteCB) {
    if let Some(win) = &mut STATE.current_window { win.paste_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutMouseFunc(func: EGLUTmouseCB) {
    if let Some(win) = &mut STATE.current_window { win.mouse_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutMouseRawFunc(func: EGLUTmouseRawCB) {
    if let Some(win) = &mut STATE.current_window { win.mouse_raw_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutMouseButtonFunc(func: EGLUTmouseButtonCB) {
    if let Some(win) = &mut STATE.current_window { win.mouse_button_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutTouchStartFunc(func: EGLUTtouchStartCB) {
    if let Some(win) = &mut STATE.current_window { win.touch_start_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutTouchUpdateFunc(func: EGLUTtouchUpdateCB) {
    if let Some(win) = &mut STATE.current_window { win.touch_update_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutTouchEndFunc(func: EGLUTtouchEndCB) {
    if let Some(win) = &mut STATE.current_window { win.touch_end_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutFocusFunc(func: EGLUTfocusCB) {
    if let Some(win) = &mut STATE.current_window { win.focus_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutCloseWindowFunc(func: EGLUTcloseCB) {
    if let Some(win) = &mut STATE.current_window { win.close_cb = func; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutSetKeyboardState(active: i32) {
    if let Some(win) = &mut STATE.current_window { win.keyboardstate = active; }
}

#[no_mangle]
pub unsafe extern "C" fn eglutIdleFunc(func: EGLUTidleCB) {
    STATE.idle_cb = func;
}

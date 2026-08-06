//! Port of the C++ `CorePatches` (Phase 2 of PORT_FAKE_LOOPER.md).
//!
//! Rust owns the `GameWindowHandle` state, the `on_window_created_callbacks`
//! registry, and the 9 `game_window_*` symbols registered into the
//! `libmcpelauncher_gamewindow.so` Rust linker stub. The `callbacks` token stays
//! an opaque pointer to the C++ `WindowCallbacks`; the `game_window_*` handlers
//! forward through small `window_callbacks_*` extern "C" helpers (still backed
//! by C++ until Phase 3). The vtable patching (`core_patches_install_impl`) and
//! the swap-buffers callback registry (`fake_egl_add_swap_buffers_callback`)
//! already lived in Rust and are reused directly.

use std::ffi::c_void;

type KeyboardCallback = extern "C" fn(*mut c_void, i32, i32) -> bool;
type MouseButtonCallback = extern "C" fn(*mut c_void, f64, f64, i32, i32) -> bool;
type MousePositionCallback = extern "C" fn(*mut c_void, f64, f64, bool) -> bool;
type MouseScrollCallback = extern "C" fn(*mut c_void, f64, f64, f64, f64) -> bool;
type SwapBuffersCallback = unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void);

// C++ `WindowCallbacks` helpers (window_callbacks_stub.cpp, removed in Phase 3)
// and the already-Rust swap-buffers registry (rust_bridge.rs).
extern "C" {
    fn window_callbacks_get_input_mode(callbacks: *mut c_void) -> i32;
    fn window_callbacks_set_cursor_locked(callbacks: *mut c_void, locked: bool);
    fn window_callbacks_set_fullscreen(callbacks: *mut c_void, fs: bool);
    fn window_callbacks_set_delayed_paste(callbacks: *mut c_void);
    fn window_callbacks_add_keyboard_callback(callbacks: *mut c_void, user: *mut c_void, cb: KeyboardCallback);
    fn window_callbacks_add_mouse_button_callback(callbacks: *mut c_void, user: *mut c_void, cb: MouseButtonCallback);
    fn window_callbacks_add_mouse_position_callback(callbacks: *mut c_void, user: *mut c_void, cb: MousePositionCallback);
    fn window_callbacks_add_mouse_scroll_callback(callbacks: *mut c_void, user: *mut c_void, cb: MouseScrollCallback);
    fn fake_egl_add_swap_buffers_callback(user: *mut c_void, cb: Option<SwapBuffersCallback>);
    fn core_patches_install_impl(handle: *mut c_void);
}

/// Opaque handle returned to the game by `game_window_get_primary_window`.
/// Raw tokens only — the C++ `FakeLooper` still owns `GameWindow`/`WindowCallbacks`
/// through Phase 4; Rust mirrors the plan's "stable-address `GameWindowHandle`".
#[repr(C)]
pub struct GameWindowHandle {
    #[allow(dead_code)]
    window: *mut c_void,
    callbacks: *mut c_void,
    mouse_locked: bool,
}

static mut HANDLE: *mut GameWindowHandle = std::ptr::null_mut();
static HANDLE_ONCE: std::sync::Once = std::sync::Once::new();

fn handle() -> *mut GameWindowHandle {
    HANDLE_ONCE.call_once(|| unsafe {
        HANDLE = Box::into_raw(Box::new(GameWindowHandle {
            window: std::ptr::null_mut(),
            callbacks: std::ptr::null_mut(),
            mouse_locked: false,
        }));
    });
    unsafe { HANDLE }
}

/// Registry filled by `game_window_add_window_creation_callback`, dispatched by
/// `core_patches_set_game_window_callbacks` (was `std::vector<std::function<void()>>`).
#[derive(Clone)]
struct WindowCreatedCallback {
    user: *mut c_void,
    cb: extern "C" fn(*mut c_void),
}
unsafe impl Send for WindowCreatedCallback {}

static ON_WINDOW_CREATED_CALLBACKS: std::sync::Mutex<Vec<WindowCreatedCallback>> =
    std::sync::Mutex::new(Vec::new());

// ============================================================
// CorePatches state (replaces core_patches_stub.cpp member functions)
// ============================================================

#[no_mangle]
pub unsafe extern "C" fn core_patches_show_mouse_pointer() {
    let h = handle();
    unsafe { (*h).mouse_locked = false };
    let cb = unsafe { (*h).callbacks };
    if !cb.is_null() {
        unsafe { window_callbacks_set_cursor_locked(cb, false) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn core_patches_hide_mouse_pointer() {
    let h = handle();
    unsafe { (*h).mouse_locked = true };
    let cb = unsafe { (*h).callbacks };
    if !cb.is_null() {
        unsafe { window_callbacks_set_cursor_locked(cb, true) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn core_patches_is_mouse_locked() -> bool {
    unsafe { (*handle()).mouse_locked }
}

#[no_mangle]
pub unsafe extern "C" fn core_patches_set_fullscreen(_t: *mut c_void, fs: bool) {
    let cb = unsafe { (*handle()).callbacks };
    if !cb.is_null() {
        unsafe { window_callbacks_set_fullscreen(cb, fs) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn core_patches_set_pending_delayed_paste() {
    let cb = unsafe { (*handle()).callbacks };
    if !cb.is_null() {
        unsafe { window_callbacks_set_delayed_paste(cb) };
    }
}

/// Replaces `CorePatches::setGameWindow`. Raw token only — the C++ side keeps
/// ownership of the `GameWindow` (via `FakeLooper`'s shared_ptr).
#[no_mangle]
pub unsafe extern "C" fn core_patches_set_game_window(window: *mut c_void) {
    unsafe { (*handle()).window = window };
}

/// Replaces `CorePatches::setGameWindowCallbacks`: stores the opaque
/// `WindowCallbacks` token and fires the registered window-creation callbacks
/// (snapshot taken under the lock; same set as the C++ vector at dispatch time).
#[no_mangle]
pub unsafe extern "C" fn core_patches_set_game_window_callbacks(callbacks: *mut c_void) {
    unsafe { (*handle()).callbacks = callbacks };
    let snapshot: Vec<WindowCreatedCallback> = {
        let guard = ON_WINDOW_CREATED_CALLBACKS.lock().unwrap();
        guard.clone()
    };
    for cb in snapshot {
        (cb.cb)(cb.user);
    }
}

#[no_mangle]
pub unsafe extern "C" fn core_patches_install(handle: *mut c_void) {
    unsafe { core_patches_install_impl(handle) };
}

// ============================================================
// game_window_* symbols (registered via mc_register_game_window_symbols)
// ============================================================

#[no_mangle]
pub unsafe extern "C" fn game_window_get_primary_window() -> *mut GameWindowHandle {
    handle()
}

#[no_mangle]
pub unsafe extern "C" fn game_window_is_mouse_locked(handle: *mut GameWindowHandle) -> bool {
    if handle.is_null() {
        return false;
    }
    unsafe { (*handle).mouse_locked }
}

#[no_mangle]
pub unsafe extern "C" fn game_window_get_input_mode(handle: *mut GameWindowHandle) -> i32 {
    if handle.is_null() {
        return 0;
    }
    let cb = unsafe { (*handle).callbacks };
    if cb.is_null() {
        return 0;
    }
    unsafe { window_callbacks_get_input_mode(cb) }
}

#[no_mangle]
pub unsafe extern "C" fn game_window_add_keyboard_callback(
    handle: *mut GameWindowHandle,
    user: *mut c_void,
    callback: KeyboardCallback,
) {
    if handle.is_null() {
        return;
    }
    let cb = unsafe { (*handle).callbacks };
    if !cb.is_null() {
        unsafe { window_callbacks_add_keyboard_callback(cb, user, callback) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn game_window_add_mouse_button_callback(
    handle: *mut GameWindowHandle,
    user: *mut c_void,
    callback: MouseButtonCallback,
) {
    if handle.is_null() {
        return;
    }
    let cb = unsafe { (*handle).callbacks };
    if !cb.is_null() {
        unsafe { window_callbacks_add_mouse_button_callback(cb, user, callback) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn game_window_add_mouse_position_callback(
    handle: *mut GameWindowHandle,
    user: *mut c_void,
    callback: MousePositionCallback,
) {
    if handle.is_null() {
        return;
    }
    let cb = unsafe { (*handle).callbacks };
    if !cb.is_null() {
        unsafe { window_callbacks_add_mouse_position_callback(cb, user, callback) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn game_window_add_mouse_scroll_callback(
    handle: *mut GameWindowHandle,
    user: *mut c_void,
    callback: MouseScrollCallback,
) {
    if handle.is_null() {
        return;
    }
    let cb = unsafe { (*handle).callbacks };
    if !cb.is_null() {
        unsafe { window_callbacks_add_mouse_scroll_callback(cb, user, callback) };
    }
}

#[no_mangle]
pub unsafe extern "C" fn game_window_add_window_creation_callback(
    user: *mut c_void,
    on_created: extern "C" fn(*mut c_void),
) {
    let mut guard = ON_WINDOW_CREATED_CALLBACKS.lock().unwrap();
    guard.push(WindowCreatedCallback { user, cb: on_created });
}

#[no_mangle]
pub unsafe extern "C" fn game_window_add_swap_buffers_callback(
    user: *mut c_void,
    callback: SwapBuffersCallback,
) {
    unsafe { fake_egl_add_swap_buffers_callback(user, Some(callback)) };
}

// ============================================================
// Startup registration (replaces CorePatches::loadGameWindowLibrary)
// ============================================================

/// Mirrors the 9 `game_window_*` symbols into the Rust linker's
/// `libmcpelauncher_gamewindow.so` stub. Must run after
/// `linker::register_stub("libmcpelauncher_gamewindow.so", ...)` (capi.rs) —
/// kept inside `capi::setup_android_hooks` to preserve that ordering.
#[no_mangle]
pub unsafe extern "C" fn mc_register_game_window_symbols() {
    let syms: [(&str, *mut c_void); 9] = [
        ("game_window_get_primary_window", game_window_get_primary_window as *mut c_void),
        ("game_window_is_mouse_locked", game_window_is_mouse_locked as *mut c_void),
        ("game_window_get_input_mode", game_window_get_input_mode as *mut c_void),
        ("game_window_add_keyboard_callback", game_window_add_keyboard_callback as *mut c_void),
        ("game_window_add_mouse_button_callback", game_window_add_mouse_button_callback as *mut c_void),
        ("game_window_add_mouse_position_callback", game_window_add_mouse_position_callback as *mut c_void),
        ("game_window_add_mouse_scroll_callback", game_window_add_mouse_scroll_callback as *mut c_void),
        ("game_window_add_window_creation_callback", game_window_add_window_creation_callback as *mut c_void),
        ("game_window_add_swap_buffers_callback", game_window_add_swap_buffers_callback as *mut c_void),
    ];
    let names: Vec<std::ffi::CString> = syms
        .iter()
        .map(|(n, _)| std::ffi::CString::new(*n).unwrap())
        .collect();
    let name_ptrs: Vec<*const std::ffi::c_char> = names.iter().map(|n| n.as_ptr()).collect();
    let val_ptrs: Vec<*mut c_void> = syms.iter().map(|(_, v)| *v).collect();
    unsafe {
        linker::linker_add_symbols_to_library_rust(
            c"libmcpelauncher_gamewindow.so".as_ptr(),
            name_ptrs.as_ptr(),
            val_ptrs.as_ptr(),
            name_ptrs.len(),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static CREATED_COUNT: AtomicUsize = AtomicUsize::new(0);

    extern "C" fn count_created(user: *mut c_void) {
        CREATED_COUNT.fetch_add(unsafe { user as usize }, Ordering::SeqCst);
    }

    #[test]
    fn handle_address_is_stable() {
        let a = unsafe { game_window_get_primary_window() };
        let b = unsafe { game_window_get_primary_window() };
        assert!(!a.is_null());
        assert_eq!(a, b);
    }

    #[test]
    fn mouse_lock_flag_toggles() {
        unsafe {
            core_patches_hide_mouse_pointer();
            assert!(core_patches_is_mouse_locked());
            core_patches_show_mouse_pointer();
            assert!(!core_patches_is_mouse_locked());
        }
    }

    #[test]
    fn window_creation_callbacks_fire_on_set_callbacks() {
        unsafe {
            CREATED_COUNT.store(0, Ordering::SeqCst);
            let h = handle();
            // callbacks non-null (dummy token) so the registry fires
            game_window_add_window_creation_callback(1 as *mut c_void, count_created);
            game_window_add_window_creation_callback(1 as *mut c_void, count_created);
            core_patches_set_game_window_callbacks(h as *mut c_void);
            assert_eq!(CREATED_COUNT.load(Ordering::SeqCst), 2);
            assert_eq!((*h).callbacks, h as *mut c_void);
        }
    }
}

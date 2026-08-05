//! Port of the C++ `FakeLooper` (Phase 4 of PORT_FAKE_LOOPER.md).
//!
//! Rust owns all looper state (per-thread `LooperState`), the `libandroid.so`
//! looper hooks, and the `prepare` orchestration that was split across
//! `fake_looper_stub.cpp` + the 17 `fake_looper_*` extern "C" helpers. The
//! window token (the game's `ANativeWindow`/`GameWindow*`) is the Rust eglut
//! X11 window id, owned by `crate::game_window` (Phase 5).
//!
//! The `FakeInputQueue` is a Rust-owned `Box` stored in `LooperState`; the
//! game-visible `AInputQueue*` handed to `JniSupport::onWindowCreated` IS that
//! pointer, and the `libandroid.so` input hooks in `fake_inputqueue.rs`
//! resolve it by identity cast.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;

#[derive(Copy, Clone)]
#[repr(C)]
struct EventEntry {
    fd: i32,
    ident: i32,
    events: i32,
    data: *mut c_void,
}

impl EventEntry {
    const fn invalid() -> Self {
        EventEntry { fd: 0, ident: -1, events: 0, data: std::ptr::null_mut() }
    }
    fn is_valid(&self) -> bool {
        self.ident != -1
    }
    unsafe fn fill(&self, out_fd: *mut i32, out_data: *mut *mut c_void) {
        if !out_fd.is_null() {
            *out_fd = self.fd;
        }
        if !out_data.is_null() {
            *out_data = self.data;
        }
    }
}

/// Per-thread looper state (replaces `thread_local std::unique_ptr<FakeLooper>`).
#[derive(Clone)]
struct LooperState {
    prepared: bool,
    window: *mut c_void,
    window_callbacks: *mut c_void,
    input_queue: *mut crate::fake_inputqueue::FakeInputQueue,
    android_event: EventEntry,
    input_entry: EventEntry,
    text_input: bool,
}

/// Mirrors `FakeLooper::~FakeLooper`: unregister core patches, destroy the
/// WindowCallbacks box, and free the Rust input queue. The GameWindow itself
/// is owned by the process-lifetime `shared_ptr` in `jni_bridge_stub.cpp`.
impl Drop for LooperState {
    fn drop(&mut self) {
        unsafe {
            crate::core_patches::core_patches_set_game_window(std::ptr::null_mut());
            if !self.window_callbacks.is_null() {
                crate::core_patches::core_patches_set_game_window_callbacks(std::ptr::null_mut());
                crate::window_callbacks::window_callbacks_destroy(self.window_callbacks);
            }
            if !self.input_queue.is_null() {
                crate::fake_inputqueue::mc_fake_input_queue_destroy(self.input_queue);
            }
        }
    }
}

thread_local! {
    static CURRENT: RefCell<Option<LooperState>> = const { RefCell::new(None) };
}

// Matches android/looper.h — NOT -1 (that is ALOOPER_POLL_WAKE).
const ALOOPER_POLL_TIMEOUT: i32 = -3;

/// Stable non-null token returned to the game from `ALooper_prepare` (the
/// hooks ignore the looper argument and use thread-local state instead).
static LOOPER_SENTINEL: u8 = 0;

extern "C" {
    fn mc_register_android_hook(map: *mut c_void, name: *const i8, fn_ptr: *mut c_void);
    fn fake_looper_finish(native: *mut c_void);

    // C++ FFI helpers for prepare (JniSupport side only; window helpers are
    // now Rust — `crate::game_window`)
    fn mc_set_looper_running_cpp(running: bool);
    fn mc_jni_support_on_window_created_cpp(window: *mut c_void, queue: *mut c_void);
    fn mc_get_jni_support() -> *mut c_void;
    fn mc_get_rust_jni_support() -> *mut c_void;
    fn fake_looper_splitscreen_patch_gl_created();
    fn fake_looper_shader_error_patch_gl_created();
}

/// Active WindowCallbacks token for the current thread, resolved from looper
/// state. Used by `window_callbacks.rs` eglut trampolines (which run on the
/// same game thread that called `prepare`/`pollAll`).
pub fn current_callbacks() -> *mut c_void {
    CURRENT.with(|c| {
        c.borrow()
            .as_ref()
            .map(|s| s.window_callbacks)
            .unwrap_or(std::ptr::null_mut())
    })
}

#[no_mangle]
pub unsafe extern "C" fn mc_register_fake_looper_hooks(map: *mut c_void) {
    mc_register_android_hook(map, c"ALooper_prepare".as_ptr(), fake_looper_hook_prepare as *mut c_void);
    mc_register_android_hook(map, c"ALooper_addFd".as_ptr(), fake_looper_hook_add_fd as *mut c_void);
    mc_register_android_hook(map, c"ALooper_pollAll".as_ptr(), fake_looper_hook_poll_all as *mut c_void);
    mc_register_android_hook(map, c"ALooper_pollOnce".as_ptr(), fake_looper_hook_poll_once as *mut c_void);
    mc_register_android_hook(map, c"AInputQueue_attachLooper".as_ptr(), fake_looper_hook_attach_input_queue as *mut c_void);
    mc_register_android_hook(map, c"ANativeActivity_finish".as_ptr(), fake_looper_hook_finish as *mut c_void);
}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_hook_prepare() -> *mut c_void {
    prepare_impl()
}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_hook_add_fd(
    _looper: *mut c_void, fd: i32, ident: i32, events: i32,
    callback: *mut c_void, data: *mut c_void,
) -> i32 {
    add_fd_impl(fd, ident, events, callback, data)
}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_hook_poll_all(
    timeout: i32, out_fd: *mut i32, out_events: *mut i32, out_data: *mut *mut c_void,
) -> i32 {
    poll_all_impl(timeout, out_fd, out_events, out_data)
}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_hook_poll_once(
    timeout: i32, out_fd: *mut i32, out_events: *mut i32, out_data: *mut *mut c_void,
) -> i32 {
    poll_all_impl(timeout, out_fd, out_events, out_data)
}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_hook_attach_input_queue(
    _queue: *mut c_void, _looper: *mut c_void, ident: i32,
    callback: *mut c_void, data: *mut c_void,
) {
    attach_input_queue_impl(ident, callback, data)
}

#[no_mangle]
pub unsafe extern "C" fn fake_looper_hook_finish(native: *mut c_void) {
    fake_looper_finish(native)
}

// --- Rust implementation of prepare ---

/// Replaces `FakeLooper::initializeWindow` + the `fake_looper_prepare_begin` /
/// `fake_looper_notify_window_created` / `fake_looper_create_window_callbacks` /
/// `fake_looper_register_core_patches` / `fake_looper_show_window` /
/// `fake_looper_window_make_current` C++ helpers.
unsafe fn prepare_impl() -> *mut c_void {
    let (window, queue) = CURRENT.with(|c| {
        let mut cur = c.borrow_mut();
        if cur.is_some() {
            panic!("Looper already prepared");
        }
        let window = initialize_window();
        let queue = crate::fake_inputqueue::mc_fake_input_queue_create();
        *cur = Some(LooperState {
            prepared: true,
            window,
            window_callbacks: std::ptr::null_mut(),
            input_queue: queue,
            android_event: EventEntry::invalid(),
            input_entry: EventEntry::invalid(),
            text_input: false,
        });
        (window, queue)
    });

    mc_set_looper_running_cpp(true);
    mc_jni_support_on_window_created_cpp(window, queue as *mut c_void);
    let rust_support = mc_get_rust_jni_support();
    if !rust_support.is_null() {
        crate::jni_support::jni_support_on_window_created(rust_support, window, queue as *mut c_void);
    }

    let callbacks = crate::window_callbacks::window_callbacks_create(
        window,
        mc_get_jni_support(),
        rust_support,
        queue as *mut c_void,
    );
    CURRENT.with(|c| {
        c.borrow_mut().as_mut().unwrap().window_callbacks = callbacks;
    });
    crate::window_callbacks::window_callbacks_register(callbacks);

    crate::core_patches::core_patches_set_game_window(window);
    crate::core_patches::core_patches_set_game_window_callbacks(callbacks);

    crate::game_window::mc_window_show(window);
    fake_looper_splitscreen_patch_gl_created();
    fake_looper_shader_error_patch_gl_created();
    crate::game_window::game_window_make_current(window, 0);

    &LOOPER_SENTINEL as *const u8 as *mut c_void
}

/// Mirrors `FakeLooper::initializeWindow`:
/// - the process-lifetime window token from `mc_create_window_and_setup_graphics`
///   (via `mc_get_window_token`), or
/// - the fallback window created by `mc_create_default_window`
///   (loads gamepad mappings first, like the C++ path did).
unsafe fn initialize_window() -> *mut c_void {
    let token = crate::game_window::mc_get_window_token();
    if !token.is_null() {
        return token;
    }
    crate::game_window::mc_create_default_window()
}

// --- Rust implementations of addFd, attachInputQueue, pollAll ---

unsafe fn add_fd_impl(fd: i32, ident: i32, events: i32, callback: *mut c_void, data: *mut c_void) -> i32 {
    if !callback.is_null() {
        panic!("callback is not supported");
    }
    CURRENT.with(|c| {
        let mut cur = c.borrow_mut();
        match cur.as_mut() {
            Some(s) => {
                if s.android_event.is_valid() {
                    return -1;
                }
                s.android_event = EventEntry { fd, ident, events, data };
                1
            }
            None => -1,
        }
    })
}

unsafe fn attach_input_queue_impl(ident: i32, callback: *mut c_void, data: *mut c_void) {
    if !callback.is_null() {
        panic!("callback is not supported");
    }
    CURRENT.with(|c| {
        let mut cur = c.borrow_mut();
        match cur.as_mut() {
            Some(s) => {
                if s.input_entry.is_valid() {
                    panic!("attachInputQueue already called on this looper");
                }
                s.input_entry = EventEntry { fd: -1, ident, events: 0, data };
            }
            None => panic!("looper not prepared"),
        }
    });
}

/// True when the game requested text input. Reads the Rust `TextInputHandler`
/// global (replaces `FakeLooper::getJniSupport()->getTextInputHandler()`).
fn text_input_enabled() -> bool {
    let h = unsafe { crate::jnivm_globals::jnivm_get_text_input_handler() };
    if h.is_null() {
        return false;
    }
    unsafe { crate::text_input_handler::text_input_handler_is_enabled(h) }
}

/// Updates the text-input latch; returns `Some(enabled)` when the state
/// changed (and the caller should start/stop window text input), else `None`.
fn sync_text_input(state: &mut LooperState, enabled: bool) -> Option<bool> {
    if state.text_input != enabled {
        state.text_input = enabled;
        Some(enabled)
    } else {
        None
    }
}

unsafe fn poll_all_impl(_timeout: i32, out_fd: *mut i32, out_events: *mut i32, out_data: *mut *mut c_void) -> i32 {
    let (callbacks, window) = CURRENT.with(|c| {
        let cur = c.borrow();
        match cur.as_ref() {
            Some(s) => (s.window_callbacks, s.window),
            None => (std::ptr::null_mut(), std::ptr::null_mut()),
        }
    });

    if !callbacks.is_null() {
        crate::window_callbacks::window_callbacks_start_send_events(callbacks);
    }

    let text_input_enabled = text_input_enabled();
    let action = CURRENT.with(|c| {
        let mut cur = c.borrow_mut();
        match cur.as_mut() {
            Some(s) => sync_text_input(s, text_input_enabled),
            None => None,
        }
    });
    if let Some(enabled) = action {
        if !window.is_null() {
            if enabled {
                crate::game_window::fake_looper_window_start_text_input(window);
            } else {
                crate::game_window::fake_looper_window_stop_text_input(window);
            }
        }
    }

    let (ae, ie, queue) = CURRENT.with(|c| {
        let cur = c.borrow();
        let s = cur.as_ref();
        (
            s.map(|s| s.android_event),
            s.map(|s| s.input_entry),
            s.map(|s| s.input_queue),
        )
    });

    // 1. Check android event fd (non-blocking poll with timeout=0)
    if let Some(ae) = ae {
        if ae.is_valid() {
            let mut fds = libc::pollfd {
                fd: ae.fd,
                events: ae.events as i16,
                revents: 0,
            };
            if libc::poll(&mut fds, 1, 0) > 0 {
                ae.fill(out_fd, out_data);
                if !out_events.is_null() {
                    *out_events = fds.revents as i32;
                }
                return ae.ident;
            }
        }
    }

    // 2. Check input queue for pending events
    if let Some(ie) = ie {
        if ie.is_valid() {
            if let Some(q) = queue {
                if !q.is_null() && (&*q).has_events() {
                    ie.fill(out_fd, out_data);
                    return ie.ident;
                }
            }
        }
    }

    // 3. Drain X11 events into the input queue
    if !window.is_null() {
        crate::game_window::fake_looper_window_poll_events(window);
    }
    if !callbacks.is_null() {
        crate::window_callbacks::window_callbacks_mark_requeue_gamepad(callbacks);
    }
    ALOOPER_POLL_TIMEOUT
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_state(f: impl FnOnce(&mut LooperState)) {
        CURRENT.with(|c| {
            if c.borrow().is_none() {
                *c.borrow_mut() = Some(LooperState {
                    prepared: true,
                    window: std::ptr::null_mut(),
                    window_callbacks: std::ptr::null_mut(),
                    input_queue: std::ptr::null_mut(),
                    android_event: EventEntry::invalid(),
                    input_entry: EventEntry::invalid(),
                    text_input: false,
                });
            }
            f(c.borrow_mut().as_mut().unwrap());
        });
    }

    fn current() -> LooperState {
        CURRENT.with(|c| c.borrow().as_ref().unwrap().clone())
    }

    #[test]
    fn event_entry_fill_and_valid() {
        let e = EventEntry::invalid();
        assert!(!e.is_valid());
        let valid = EventEntry { fd: 7, ident: 5, events: 0, data: 3 as *mut c_void };
        assert!(valid.is_valid());
        let mut out_fd = -1;
        let mut out_data: *mut c_void = std::ptr::null_mut();
        unsafe { valid.fill(&mut out_fd, &mut out_data) };
        assert_eq!(out_fd, 7);
        assert_eq!(out_data, 3 as *mut c_void);
    }

    #[test]
    fn add_fd_rejects_second() {
        with_state(|_| {});
        unsafe {
            assert_eq!(add_fd_impl(1, 2, 3, std::ptr::null_mut(), std::ptr::null_mut()), 1);
            assert_eq!(add_fd_impl(4, 5, 6, std::ptr::null_mut(), std::ptr::null_mut()), -1);
        }
        let s = current();
        assert!(s.android_event.is_valid());
        assert_eq!(s.android_event.fd, 1);
        assert_eq!(s.android_event.ident, 2);
    }

    #[test]
    fn attach_input_queue_sets_entry() {
        with_state(|s| s.input_entry = EventEntry::invalid());
        unsafe { attach_input_queue_impl(11, std::ptr::null_mut(), 5 as *mut c_void) };
        let s = current();
        let ie = s.input_entry;
        assert!(ie.is_valid());
        assert_eq!(ie.ident, 11);
        assert_eq!(ie.data, 5 as *mut c_void);
        assert_eq!(ie.fd, -1);
    }

    #[test]
    fn poll_all_empty_returns_timeout() {
        with_state(|_| {});
        unsafe {
            assert_eq!(
                poll_all_impl(0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()),
                ALOOPER_POLL_TIMEOUT
            );
        }
    }

    #[test]
    fn poll_all_returns_input_entry_when_queue_has_events() {
        let queue = unsafe { crate::fake_inputqueue::mc_fake_input_queue_create() };
        assert!(!queue.is_null());
        let key = crate::fake_inputqueue::FakeKeyEvent::new(1, 2, 0);
        unsafe { crate::fake_inputqueue::mc_fake_input_queue_add_key_event(queue, &key) };
        with_state(|s| {
            s.input_queue = queue;
            s.input_entry = EventEntry { fd: -1, ident: 42, events: 0, data: 0xdead as *mut c_void };
        });
        let mut out_fd = -99;
        let mut out_data: *mut c_void = std::ptr::null_mut();
        let r = unsafe { poll_all_impl(0, &mut out_fd, std::ptr::null_mut(), &mut out_data) };
        assert_eq!(r, 42);
        assert_eq!(out_fd, -1);
        assert_eq!(out_data, 0xdead as *mut c_void);
    }

    #[test]
    fn poll_all_skips_invalid_android_fd() {
        // fd=-1 makes libc::poll return <= 0 (POLLNVAL/ignored), so the
        // android event is skipped and the input entry is returned instead.
        let queue = unsafe { crate::fake_inputqueue::mc_fake_input_queue_create() };
        let key = crate::fake_inputqueue::FakeKeyEvent::new(1, 2, 0);
        unsafe { crate::fake_inputqueue::mc_fake_input_queue_add_key_event(queue, &key) };
        with_state(|s| {
            s.android_event = EventEntry { fd: -1, ident: 7, events: 0, data: std::ptr::null_mut() };
            s.input_queue = queue;
            s.input_entry = EventEntry { fd: -1, ident: 9, events: 0, data: std::ptr::null_mut() };
        });
        let r = unsafe { poll_all_impl(0, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut()) };
        assert_eq!(r, 9);
    }

    #[test]
    fn text_input_latch_transitions() {
        let mut s = LooperState {
            prepared: true,
            window: std::ptr::null_mut(),
            window_callbacks: std::ptr::null_mut(),
            input_queue: std::ptr::null_mut(),
            android_event: EventEntry::invalid(),
            input_entry: EventEntry::invalid(),
            text_input: false,
        };
        assert_eq!(sync_text_input(&mut s, true), Some(true));
        assert!(s.text_input);
        assert_eq!(sync_text_input(&mut s, true), None);
        assert_eq!(sync_text_input(&mut s, false), Some(false));
        assert!(!s.text_input);
    }
}

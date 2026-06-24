use std::cell::Cell;
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

thread_local! {
    static ANDROID_EVENT: Cell<EventEntry> = const { Cell::new(EventEntry::invalid()) };
    static INPUT_ENTRY: Cell<EventEntry> = const { Cell::new(EventEntry::invalid()) };
    static TEXT_INPUT: Cell<bool> = const { Cell::new(false) };
}

const ALOOPER_POLL_TIMEOUT: i32 = -1;

extern "C" {
    fn mc_register_android_hook(map: *mut c_void, name: *const i8, fn_ptr: *mut c_void);
    fn fake_looper_finish(native: *mut c_void);

    // C++ FFI helpers for prepare
    fn fake_looper_prepare_begin() -> *mut c_void;
    fn fake_looper_notify_window_created();
    fn fake_looper_create_window_callbacks();
    fn fake_looper_register_core_patches();
    fn fake_looper_show_window();
    fn fake_looper_splitscreen_patch_gl_created();
    fn fake_looper_shader_error_patch_gl_created();
    fn fake_looper_window_make_current(v: i32);

    // C++ FFI helpers for pollAll
    fn fake_looper_get_window() -> *mut c_void;
    fn fake_looper_get_callbacks() -> *mut c_void;
    fn fake_looper_get_input_queue() -> *mut c_void;
    fn fake_looper_get_text_input_enabled() -> bool;
    fn fake_looper_callbacks_start_send_events(callbacks: *mut c_void);
    fn fake_looper_callbacks_mark_requeue_gamepad(callbacks: *mut c_void);
    fn fake_looper_window_poll_events(window: *mut c_void);
    fn fake_looper_window_start_text_input(window: *mut c_void);
    fn fake_looper_window_stop_text_input(window: *mut c_void);
    fn fake_input_queue_has_events(queue: *mut c_void) -> bool;
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

unsafe fn prepare_impl() -> *mut c_void {
    eprintln!("=== FakeLooper::prepare: Rust ===");
    let looper = fake_looper_prepare_begin();
    eprintln!("=== FakeLooper::prepare: initializeWindow done ===");
    eprintln!("=== FakeLooper::prepare: onWindowCreated window={:?} ===",
              fake_looper_get_window());
    fake_looper_notify_window_created();
    eprintln!("=== FakeLooper::prepare: creating WindowCallbacks ===");
    fake_looper_create_window_callbacks();
    fake_looper_register_core_patches();
    fake_looper_show_window();
    fake_looper_splitscreen_patch_gl_created();
    fake_looper_shader_error_patch_gl_created();
    eprintln!("=== FakeLooper::prepare: makeCurrent(false) ===");
    fake_looper_window_make_current(0);
    eprintln!("=== FakeLooper::prepare: done ===");
    looper
}

// --- Rust implementations of addFd, attachInputQueue, pollAll ---

unsafe fn add_fd_impl(fd: i32, ident: i32, events: i32, callback: *mut c_void, data: *mut c_void) -> i32 {
    if !callback.is_null() {
        panic!("callback is not supported");
    }
    ANDROID_EVENT.with(|ae| {
        if ae.get().is_valid() {
            return -1;
        }
        ae.set(EventEntry { fd, ident, events, data });
        1
    })
}

unsafe fn attach_input_queue_impl(ident: i32, callback: *mut c_void, data: *mut c_void) {
    if !callback.is_null() {
        panic!("callback is not supported");
    }
    INPUT_ENTRY.with(|ie| {
        if ie.get().is_valid() {
            panic!("attachInputQueue already called on this looper");
        }
        ie.set(EventEntry { fd: -1, ident, events: 0, data });
    });
}

unsafe fn poll_all_impl(_timeout: i32, out_fd: *mut i32, out_events: *mut i32, out_data: *mut *mut c_void) -> i32 {
    let callbacks = fake_looper_get_callbacks();
    if !callbacks.is_null() {
        fake_looper_callbacks_start_send_events(callbacks);
    }

    let text_input_enabled = fake_looper_get_text_input_enabled();
    TEXT_INPUT.with(|ti| {
        if ti.get() != text_input_enabled {
            ti.set(text_input_enabled);
            let window = fake_looper_get_window();
            if text_input_enabled {
                fake_looper_window_start_text_input(window);
            } else {
                fake_looper_window_stop_text_input(window);
            }
        }
    });

    let ae = ANDROID_EVENT.with(|ae| ae.get());
    let ie = INPUT_ENTRY.with(|ie| ie.get());

    // 1. Check android event fd (non-blocking poll with timeout=0)
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

    // 2. Check input queue for pending events
    if ie.is_valid() {
        let queue = fake_looper_get_input_queue();
        if !queue.is_null() && fake_input_queue_has_events(queue) {
            ie.fill(out_fd, out_data);
            return ie.ident;
        }
    }

    // 3. Drain X11 events into the input queue
    let window = fake_looper_get_window();
    if !window.is_null() {
        fake_looper_window_poll_events(window);
    }
    if !callbacks.is_null() {
        fake_looper_callbacks_mark_requeue_gamepad(callbacks);
    }
    ALOOPER_POLL_TIMEOUT
}

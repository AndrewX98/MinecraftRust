use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, AtomicU64, AtomicUsize, Ordering};
use std::sync::{LazyLock, Mutex};

// === FakeWindow: ANativeWindow_getWidth/Height ===

static WINDOW_WIDTH: AtomicI32 = AtomicI32::new(1600);
static WINDOW_HEIGHT: AtomicI32 = AtomicI32::new(1200);
static MENUBAR_SIZE: AtomicI32 = AtomicI32::new(0);

#[no_mangle]
pub extern "C" fn fake_window_set_size(width: i32, height: i32) {
    WINDOW_WIDTH.store(width, Ordering::SeqCst);
    WINDOW_HEIGHT.store(height, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn fake_window_set_menubar_size(size: i32) {
    MENUBAR_SIZE.store(size, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn fake_anativewindow_getwidth(_window: *mut std::ffi::c_void) -> i32 {
    WINDOW_WIDTH.load(Ordering::SeqCst)
}

#[no_mangle]
pub unsafe extern "C" fn fake_anativewindow_getheight(_window: *mut std::ffi::c_void) -> i32 {
    (WINDOW_HEIGHT.load(Ordering::SeqCst) - MENUBAR_SIZE.load(Ordering::SeqCst)).max(1)
}

// === FakeSwappyGL: hook stubs ===

#[repr(C)]
pub struct McpelauncherHook {
    pub name: *const std::ffi::c_char,
    pub value: *mut std::ffi::c_void,
}

#[no_mangle]
pub unsafe extern "C" fn fake_swappygl_fill_hooks(
    hooks: *mut McpelauncherHook,
    count: usize,
) {
    extern "C" {
        fn mc_egl_swap_buffers(
            display: *mut std::ffi::c_void,
            surface: *mut std::ffi::c_void,
        ) -> i32;
    }

    let hooks_data = [
        (b"SwappyGL_init\0" as &[u8], stubs::swappygl_init as *mut _),
        (b"SwappyGL_destroy\0" as &[u8], stubs::swappygl_destroy as *mut _),
        (b"SwappyGL_getFenceTimeoutNS\0" as &[u8], stubs::swappygl_get_fence_timeout as *mut _),
        (b"SwappyGL_getRefreshPeriodNanos\0" as &[u8], stubs::swappygl_get_refresh_period as *mut _),
        (b"SwappyGL_getSupportedRefreshPeriodsNS\0" as &[u8], stubs::swappygl_noop_void as *mut _),
        (b"SwappyGL_getSwapIntervalNS\0" as &[u8], stubs::swappygl_noop_void as *mut _),
        (b"SwappyGL_getUseAffinity\0" as &[u8], stubs::swappygl_noop_void as *mut _),
        (b"SwappyGL_isEnabled\0" as &[u8], stubs::swappygl_is_enabled as *mut _),
        (b"SwappyGL_setBufferStuffingFixWait\0" as &[u8], stubs::swappygl_noop as *mut _),
        (b"SwappyGL_setFenceTimeoutNS\0" as &[u8], stubs::swappygl_noop as *mut _),
        (b"SwappyGL_setSwapIntervalNS\0" as &[u8], stubs::swappygl_noop as *mut _),
        (b"SwappyGL_setUseAffinity\0" as &[u8], stubs::swappygl_noop as *mut _),
        (b"SwappyGL_setWindow\0" as &[u8], stubs::swappygl_set_window as *mut _),
        (b"SwappyGL_enableFramePacing\0" as &[u8], stubs::swappygl_noop as *mut _),
        (b"SwappyGL_swap\0" as &[u8], stubs::swappygl_swap as *mut _),
    ];
    let dst = std::slice::from_raw_parts_mut(hooks, count);
    for (i, (name_bytes, value)) in hooks_data.iter().enumerate().take(count) {
        dst[i].name = name_bytes.as_ptr() as *const std::ffi::c_char;
        dst[i].value = *value;
    }
}

mod stubs {
    pub extern "C" fn swappygl_init() -> bool {
        true
    }
    pub extern "C" fn swappygl_destroy() {}
    pub extern "C" fn swappygl_get_fence_timeout() -> u64 {
        0
    }
    pub extern "C" fn swappygl_get_refresh_period() -> u64 {
        0
    }
    pub extern "C" fn swappygl_noop_void() {}
    pub extern "C" fn swappygl_noop() {}
    pub extern "C" fn swappygl_is_enabled() -> bool {
        true
    }
    pub extern "C" fn swappygl_set_window() -> bool {
        true
    }
    pub extern "C" fn swappygl_swap(
        display: *mut std::ffi::c_void,
        surface: *mut std::ffi::c_void,
    ) -> bool {
        unsafe { crate::capi::mc_egl_swap_buffers(display, surface) != 0 }
    }
}

// === ThreadMover ===

static START_THREAD_STORED: AtomicBool = AtomicBool::new(false);

#[no_mangle]
pub extern "C" fn fake_thread_mover_store_start_thread_id() {
    START_THREAD_STORED.store(true, Ordering::SeqCst);
}

#[no_mangle]
pub extern "C" fn fake_thread_mover_execute_main_thread() {
    // Block forever to keep the process alive while the game renders.
    let (_tx, rx) = std::sync::mpsc::channel::<()>();
    // Leak the sender so recv() never returns Err
    Box::leak(Box::new(_tx));
    let _ = rx.recv();
}

// === GLCorePatch: desktop GL compatibility shim ===

/// Whether the game binary has been patched to force desktop GL mode.
static GL_CORE_ENABLED: AtomicBool = AtomicBool::new(false);

/// Original GL function pointers
static GL_GEN_VERTEX_ARRAYS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static GL_BIND_VERTEX_ARRAY: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static GL_SHADER_SOURCE_ORIG: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static GL_LINK_PROGRAM_ORIG: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static GL_USE_PROGRAM_ORIG: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static GL_BIND_BUFFER_ORIG: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

/// Program → VAO mapping
static VAO_MAP: LazyLock<Mutex<HashMap<u32, u32>>> = LazyLock::new(|| Mutex::new(HashMap::new()));
/// Track bound buffers: [(GL_ARRAY_BUFFER, handle), (GL_ELEMENT_ARRAY_BUFFER, handle)]
static GL_TRACKED_BUFFERS: Mutex<[(i32, u32); 2]> = Mutex::new([(0x8892, 0), (0x8893, 0)]);

/// Desktop GL not needed on Linux.
#[no_mangle]
pub extern "C" fn mc_glcorepatch_must_use_desktop_gl() -> bool {
    false
}

/// Called to patch the game binary's `gl::supportsImmediateMode()`.
/// Not currently called from the Rust build (main.cpp excluded).
#[no_mangle]
pub extern "C" fn mc_glcorepatch_install(_handle: *mut c_void) {
    GL_CORE_ENABLED.store(true, Ordering::SeqCst);
}

/// Install GL overrides for core-profile compatibility.
/// Only active if `mc_glcorepatch_install` was called first.
///
/// `resolver`: callback to resolve GL function names to function pointers.
/// `add_override`: callback to register `(name, fn_ptr)` into the host override map.
#[no_mangle]
pub unsafe extern "C" fn mc_glcorepatch_install_gl(
    resolver: extern "C" fn(*const c_char) -> *mut c_void,
    add_override: extern "C" fn(*const c_char, *mut c_void),
) {
    if !GL_CORE_ENABLED.load(Ordering::SeqCst) {
        return;
    }

    GL_GEN_VERTEX_ARRAYS.store(resolver(c"glGenVertexArrays".as_ptr()), Ordering::SeqCst);
    GL_BIND_VERTEX_ARRAY.store(resolver(c"glBindVertexArray".as_ptr()), Ordering::SeqCst);
    GL_SHADER_SOURCE_ORIG.store(resolver(c"glShaderSource".as_ptr()), Ordering::SeqCst);
    GL_LINK_PROGRAM_ORIG.store(resolver(c"glLinkProgram".as_ptr()), Ordering::SeqCst);
    GL_USE_PROGRAM_ORIG.store(resolver(c"glUseProgram".as_ptr()), Ordering::SeqCst);
    GL_BIND_BUFFER_ORIG.store(resolver(c"glBindBuffer".as_ptr()), Ordering::SeqCst);

    add_override(c"glShaderSource".as_ptr(), mc_glcorepatch_gl_shader_source as *mut c_void);
    add_override(c"glLinkProgram".as_ptr(), mc_glcorepatch_gl_link_program as *mut c_void);
    add_override(c"glUseProgram".as_ptr(), mc_glcorepatch_gl_use_program as *mut c_void);
    add_override(c"glBindBuffer".as_ptr(), mc_glcorepatch_gl_bind_buffer as *mut c_void);
}

/// Static C string for the desktop GL version string.
static VERSION_410: &[u8] = b"#version 410\n\0";

#[no_mangle]
pub unsafe extern "C" fn mc_glcorepatch_gl_shader_source(
    shader: u32,
    count: u32,
    string: *mut *const c_char,
    length: *mut c_int,
) {
    let orig: extern "C" fn(u32, u32, *mut *const c_char, *mut c_int) =
        std::mem::transmute(GL_SHADER_SOURCE_ORIG.load(Ordering::SeqCst));

    // Replace "#version 300 es" with "#version 410"
    if !length.is_null() && *length > 0 && !string.is_null() && !(*string).is_null() {
        let s = CStr::from_ptr(*string);
        if s.to_bytes() == b"#version 300 es" {
            *string = VERSION_410.as_ptr() as *const c_char;
            *length = 12; // strlen("#version 410\n")
        }
    }

    orig(shader, count, string, length);
}

#[no_mangle]
pub unsafe extern "C" fn mc_glcorepatch_gl_link_program(program: u32) {
    let orig: extern "C" fn(u32) = std::mem::transmute(GL_LINK_PROGRAM_ORIG.load(Ordering::SeqCst));
    orig(program);

    let gen: extern "C" fn(c_int, *mut u32) =
        std::mem::transmute(GL_GEN_VERTEX_ARRAYS.load(Ordering::SeqCst));
    let bind: extern "C" fn(u32) =
        std::mem::transmute(GL_BIND_VERTEX_ARRAY.load(Ordering::SeqCst));

    let mut vertex_arr: u32 = 0;
    gen(1, &mut vertex_arr);
    bind(vertex_arr);
    VAO_MAP.lock().unwrap().insert(program, vertex_arr);
}

#[no_mangle]
pub unsafe extern "C" fn mc_glcorepatch_gl_use_program(program: u32) {
    let orig: extern "C" fn(u32) = std::mem::transmute(GL_USE_PROGRAM_ORIG.load(Ordering::SeqCst));
    orig(program);

    if program != 0 {
        let vao = *VAO_MAP.lock().unwrap().get(&program).expect("no VAO for program");
        let bind_vao: extern "C" fn(u32) =
            std::mem::transmute(GL_BIND_VERTEX_ARRAY.load(Ordering::SeqCst));
        bind_vao(vao);

        let bind_buf: extern "C" fn(c_int, u32) =
            std::mem::transmute(GL_BIND_BUFFER_ORIG.load(Ordering::SeqCst));
        for &(target, buffer) in GL_TRACKED_BUFFERS.lock().unwrap().iter() {
            bind_buf(target, buffer);
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn mc_glcorepatch_gl_bind_buffer(target: c_int, buffer: u32) {
    let orig: extern "C" fn(c_int, u32) =
        std::mem::transmute(GL_BIND_BUFFER_ORIG.load(Ordering::SeqCst));
    orig(target, buffer);

    if let Ok(mut bufs) = GL_TRACKED_BUFFERS.lock() {
        for entry in bufs.iter_mut() {
            if entry.0 == target {
                entry.1 = buffer;
                return;
            }
        }
    }
}

// === CorePatches: vtable patching ===
// Most functions are in core_patches_stub.cpp (C++). Only the vtable
// patching logic is in Rust because it uses linker::dlsym and
// PatchUtils::VtableReplaceHelper via extern "C" helpers.

/// C++ helper functions (defined in core_patches_stub.cpp)
extern "C" {
    fn core_linker_dlsym(handle: *mut c_void, sym: *const c_char) -> *mut c_void;
    fn core_vtable_replace(
        lib: *mut c_void,
        vta: *mut *mut c_void,
        name: *const c_char,
        replacement: *mut c_void,
    );
}

/// These extern "C" thunks are defined in core_patches_stub.cpp.
/// Re-declared here for Rust to take their address when needed.
extern "C" {
    fn core_patches_show_mouse_pointer();
    fn core_patches_hide_mouse_pointer();
}

/// Patch `XalInitialize` to an immediate `return S_OK` so Xbox Auth Library
/// never runs CreateGlobalState. With stubbed libHttpClient, real XAL init
/// either throws uncaught `Xal::Exception` (HCInitialize → E_FAIL) or
/// SIGSEGVs on a bad object pointer. Offline / main-menu play does not need
/// XAL; Xbox Live features remain unavailable until real HTTP+XAL work.
unsafe fn patch_xal_initialize_noop(handle: *mut c_void) {
    let sym = core_linker_dlsym(handle, c"XalInitialize".as_ptr());
    if sym.is_null() {
        log::warn!("CorePatches: XalInitialize not found — cannot soft-disable XAL");
        return;
    }
    // xor eax, eax ; ret  →  S_OK (0)
    let patch: [u8; 3] = [0x31, 0xc0, 0xc3];
    let page_size = libc::sysconf(libc::_SC_PAGESIZE) as usize;
    let addr = sym as usize;
    let page = addr & !(page_size - 1);
    let len = (addr + patch.len() + page_size - 1) & !(page_size - 1);
    let len = len - page;
    if libc::mprotect(
        page as *mut c_void,
        len,
        libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC,
    ) != 0
    {
        log::warn!(
            "CorePatches: mprotect failed for XalInitialize patch at {:p}: {}",
            sym,
            std::io::Error::last_os_error()
        );
        return;
    }
    std::ptr::copy_nonoverlapping(patch.as_ptr(), sym as *mut u8, patch.len());
    // Best-effort restore RX (ignore failure — some kernels keep W^X)
    let _ = libc::mprotect(page as *mut c_void, len, libc::PROT_READ | libc::PROT_EXEC);
    log::info!(
        "CorePatches: patched XalInitialize at {:p} → return S_OK (XAL disabled)",
        sym
    );
}

/// Installs vtable patches on the game library. Called from
/// CorePatches::install() in core_patches_stub.cpp.
#[no_mangle]
pub unsafe extern "C" fn core_patches_install_impl(handle: *mut c_void) {
    let vtable_sym = c"_ZTV21AppPlatform_android23";
    let vtable_ptr = core_linker_dlsym(handle, vtable_sym.as_ptr());
    if vtable_ptr.is_null() {
        log::warn!("CorePatches: vtable _ZTV21AppPlatform_android23 not found");
    } else {
        let vta = (vtable_ptr as *mut *mut c_void).add(2);
        // vta points to first virtual function entry (skipping typeinfo + offset)

        core_vtable_replace(
            handle,
            vta,
            c"_ZN11AppPlatform16hideMousePointerEv".as_ptr(),
            core_patches_hide_mouse_pointer as *mut c_void,
        );
        core_vtable_replace(
            handle,
            vta,
            c"_ZN11AppPlatform16showMousePointerEv".as_ptr(),
            core_patches_show_mouse_pointer as *mut c_void,
        );
    }

    patch_xal_initialize_noop(handle);
}

// === WindowCallbacks key mapping (ported from window_callbacks.cpp) ===
// These are called from window_callbacks_stub.cpp via extern "C".

mod android_keycodes {
    pub const AKEYCODE_UNKNOWN: i32 = 0;
    pub const AKEYCODE_BACK: i32 = 4;
    pub const AKEYCODE_0: i32 = 7;
    pub const AKEYCODE_1: i32 = 8;
    pub const AKEYCODE_2: i32 = 9;
    pub const AKEYCODE_3: i32 = 10;
    pub const AKEYCODE_4: i32 = 11;
    pub const AKEYCODE_5: i32 = 12;
    pub const AKEYCODE_6: i32 = 13;
    pub const AKEYCODE_7: i32 = 14;
    pub const AKEYCODE_8: i32 = 15;
    pub const AKEYCODE_9: i32 = 16;
    pub const AKEYCODE_DPAD_UP: i32 = 19;
    pub const AKEYCODE_DPAD_DOWN: i32 = 20;
    pub const AKEYCODE_DPAD_LEFT: i32 = 21;
    pub const AKEYCODE_DPAD_RIGHT: i32 = 22;
    pub const AKEYCODE_A: i32 = 29;
    pub const AKEYCODE_B: i32 = 30;
    pub const AKEYCODE_C: i32 = 31;
    pub const AKEYCODE_D: i32 = 32;
    pub const AKEYCODE_E: i32 = 33;
    pub const AKEYCODE_F: i32 = 34;
    pub const AKEYCODE_G: i32 = 35;
    pub const AKEYCODE_H: i32 = 36;
    pub const AKEYCODE_I: i32 = 37;
    pub const AKEYCODE_J: i32 = 38;
    pub const AKEYCODE_K: i32 = 39;
    pub const AKEYCODE_L: i32 = 40;
    pub const AKEYCODE_M: i32 = 41;
    pub const AKEYCODE_N: i32 = 42;
    pub const AKEYCODE_O: i32 = 43;
    pub const AKEYCODE_P: i32 = 44;
    pub const AKEYCODE_Q: i32 = 45;
    pub const AKEYCODE_R: i32 = 46;
    pub const AKEYCODE_S: i32 = 47;
    pub const AKEYCODE_T: i32 = 48;
    pub const AKEYCODE_U: i32 = 49;
    pub const AKEYCODE_V: i32 = 50;
    pub const AKEYCODE_W: i32 = 51;
    pub const AKEYCODE_X: i32 = 52;
    pub const AKEYCODE_Y: i32 = 53;
    pub const AKEYCODE_Z: i32 = 54;
    pub const AKEYCODE_COMMA: i32 = 55;
    pub const AKEYCODE_PERIOD: i32 = 56;
    pub const AKEYCODE_ALT_LEFT: i32 = 57;
    pub const AKEYCODE_ALT_RIGHT: i32 = 58;
    pub const AKEYCODE_SHIFT_LEFT: i32 = 59;
    pub const AKEYCODE_SHIFT_RIGHT: i32 = 60;
    pub const AKEYCODE_TAB: i32 = 61;
    pub const AKEYCODE_SPACE: i32 = 62;
    pub const AKEYCODE_ENTER: i32 = 66;
    pub const AKEYCODE_DEL: i32 = 67;
    pub const AKEYCODE_GRAVE: i32 = 68;
    pub const AKEYCODE_MINUS: i32 = 69;
    pub const AKEYCODE_EQUALS: i32 = 70;
    pub const AKEYCODE_LEFT_BRACKET: i32 = 71;
    pub const AKEYCODE_RIGHT_BRACKET: i32 = 72;
    pub const AKEYCODE_BACKSLASH: i32 = 73;
    pub const AKEYCODE_SEMICOLON: i32 = 74;
    pub const AKEYCODE_APOSTROPHE: i32 = 75;
    pub const AKEYCODE_SLASH: i32 = 76;
    pub const AKEYCODE_MENU: i32 = 82;
    pub const AKEYCODE_PAGE_UP: i32 = 92;
    pub const AKEYCODE_PAGE_DOWN: i32 = 93;
    pub const AKEYCODE_BUTTON_A: i32 = 96;
    pub const AKEYCODE_BUTTON_B: i32 = 97;
    pub const AKEYCODE_BUTTON_X: i32 = 99;
    pub const AKEYCODE_BUTTON_Y: i32 = 100;
    pub const AKEYCODE_BUTTON_L1: i32 = 102;
    pub const AKEYCODE_BUTTON_R1: i32 = 103;
    pub const AKEYCODE_BUTTON_THUMBL: i32 = 106;
    pub const AKEYCODE_BUTTON_THUMBR: i32 = 107;
    pub const AKEYCODE_BUTTON_START: i32 = 108;
    pub const AKEYCODE_BUTTON_SELECT: i32 = 109;
    pub const AKEYCODE_BUTTON_MODE: i32 = 110;
    pub const AKEYCODE_ESCAPE: i32 = 111;
    pub const AKEYCODE_FORWARD_DEL: i32 = 112;
    pub const AKEYCODE_CTRL_LEFT: i32 = 113;
    pub const AKEYCODE_CTRL_RIGHT: i32 = 114;
    pub const AKEYCODE_CAPS_LOCK: i32 = 115;
    pub const AKEYCODE_SCROLL_LOCK: i32 = 116;
    pub const AKEYCODE_META_LEFT: i32 = 117;
    pub const AKEYCODE_META_RIGHT: i32 = 118;
    pub const AKEYCODE_BREAK: i32 = 121;
    pub const AKEYCODE_MOVE_HOME: i32 = 122;
    pub const AKEYCODE_MOVE_END: i32 = 123;
    pub const AKEYCODE_INSERT: i32 = 124;
    pub const AKEYCODE_F1: i32 = 131;
    pub const AKEYCODE_F2: i32 = 132;
    pub const AKEYCODE_F3: i32 = 133;
    pub const AKEYCODE_F4: i32 = 134;
    pub const AKEYCODE_F5: i32 = 135;
    pub const AKEYCODE_F6: i32 = 136;
    pub const AKEYCODE_F7: i32 = 137;
    pub const AKEYCODE_F8: i32 = 138;
    pub const AKEYCODE_F9: i32 = 139;
    pub const AKEYCODE_F10: i32 = 140;
    pub const AKEYCODE_F11: i32 = 141;
    pub const AKEYCODE_F12: i32 = 142;
    pub const AKEYCODE_NUM_LOCK: i32 = 143;
    pub const AKEYCODE_NUMPAD_0: i32 = 144;
    pub const AKEYCODE_NUMPAD_1: i32 = 145;
    pub const AKEYCODE_NUMPAD_2: i32 = 146;
    pub const AKEYCODE_NUMPAD_3: i32 = 147;
    pub const AKEYCODE_NUMPAD_4: i32 = 148;
    pub const AKEYCODE_NUMPAD_5: i32 = 149;
    pub const AKEYCODE_NUMPAD_6: i32 = 150;
    pub const AKEYCODE_NUMPAD_7: i32 = 151;
    pub const AKEYCODE_NUMPAD_8: i32 = 152;
    pub const AKEYCODE_NUMPAD_9: i32 = 153;
    pub const AKEYCODE_NUMPAD_DIVIDE: i32 = 154;
    pub const AKEYCODE_NUMPAD_MULTIPLY: i32 = 155;
    pub const AKEYCODE_NUMPAD_SUBTRACT: i32 = 156;
    pub const AKEYCODE_NUMPAD_ADD: i32 = 157;
    pub const AKEYCODE_NUMPAD_DOT: i32 = 158;
}

mod android_input {
    pub const AMOTION_EVENT_BUTTON_PRIMARY: i32 = 1 << 0;
    pub const AMOTION_EVENT_BUTTON_SECONDARY: i32 = 1 << 1;
    pub const AMOTION_EVENT_BUTTON_TERTIARY: i32 = 1 << 2;
    pub const AMOTION_EVENT_BUTTON_BACK: i32 = 1 << 3;
    pub const AMOTION_EVENT_BUTTON_FORWARD: i32 = 1 << 4;
}

/// Matches KeyCode values from key_mapping.h
mod key_code {
    pub const NUM_0: i32 = 48;
    pub const NUM_9: i32 = 57;
    pub const NUMPAD_0: i32 = 0x60;
    pub const NUMPAD_9: i32 = 0x69;
    pub const NUMPAD_MULTIPLY: i32 = 0x6a;
    pub const NUMPAD_ADD: i32 = 0x6b;
    pub const NUMPAD_SEPERATOR: i32 = 0x6c;
    pub const NUMPAD_SUBTRACT: i32 = 0x6d;
    pub const NUMPAD_DECIMAL: i32 = 0x6e;
    pub const NUMPAD_DIVIDE: i32 = 0x6f;
    pub const A: i32 = 65;
    pub const Z: i32 = 90;
    pub const FN1: i32 = 112;
    pub const FN12: i32 = 123;
    pub const BACK: i32 = 4;
    pub const BACKSPACE: i32 = 8;
    pub const TAB: i32 = 9;
    pub const ENTER: i32 = 13;
    pub const LEFT_SHIFT: i32 = 16;
    pub const RIGHT_SHIFT: i32 = 16 | 256;
    pub const LEFT_CTRL: i32 = 17;
    pub const RIGHT_CTRL: i32 = 17 | 256;
    pub const PAUSE: i32 = 19;
    pub const CAPS_LOCK: i32 = 20;
    pub const ESCAPE: i32 = 27;
    pub const SPACE: i32 = 32;
    pub const PAGE_UP: i32 = 33;
    pub const PAGE_DOWN: i32 = 34;
    pub const END: i32 = 35;
    pub const HOME: i32 = 36;
    pub const LEFT: i32 = 37;
    pub const UP: i32 = 38;
    pub const RIGHT: i32 = 39;
    pub const DOWN: i32 = 40;
    pub const INSERT: i32 = 45;
    pub const DELETE: i32 = 46;
    pub const NUM_LOCK: i32 = 144;
    pub const SCROLL_LOCK: i32 = 145;
    pub const SEMICOLON: i32 = 186;
    pub const EQUAL: i32 = 187;
    pub const COMMA: i32 = 188;
    pub const MINUS: i32 = 189;
    pub const PERIOD: i32 = 190;
    pub const SLASH: i32 = 191;
    pub const GRAVE: i32 = 192;
    pub const LEFT_BRACKET: i32 = 219;
    pub const BACKSLASH: i32 = 220;
    pub const RIGHT_BRACKET: i32 = 221;
    pub const APOSTROPHE: i32 = 222;
    pub const MENU: i32 = 255;
    pub const LEFT_SUPER: i32 = 1;
    pub const RIGHT_SUPER: i32 = 1 | 256;
    pub const LEFT_ALT: i32 = 0x12;
    pub const RIGHT_ALT: i32 = 0x12 | 256;
}

use android_input::*;
use android_keycodes::*;

#[no_mangle]
pub extern "C" fn window_callbacks_map_mouse_button(btn: i32) -> i32 {
    match btn {
        1 => AMOTION_EVENT_BUTTON_PRIMARY,
        2 => AMOTION_EVENT_BUTTON_SECONDARY,
        3 => AMOTION_EVENT_BUTTON_TERTIARY,
        8 => AMOTION_EVENT_BUTTON_BACK,
        9 => AMOTION_EVENT_BUTTON_FORWARD,
        _ => btn,
    }
}

#[no_mangle]
pub extern "C" fn window_callbacks_map_minecraft_key(code: i32) -> i32 {
    if code >= key_code::NUM_0 && code <= key_code::NUM_9 {
        return code - key_code::NUM_0 + AKEYCODE_0;
    }
    if code >= key_code::NUMPAD_0 && code <= key_code::NUMPAD_9 {
        return code - key_code::NUMPAD_0 + AKEYCODE_NUMPAD_0;
    }
    if code >= key_code::A && code <= key_code::Z {
        return code - key_code::A + AKEYCODE_A;
    }
    if code >= key_code::FN1 && code <= key_code::FN12 {
        return code - key_code::FN1 + AKEYCODE_F1;
    }
    match code {
        key_code::BACK => AKEYCODE_BACK,
        key_code::BACKSPACE => AKEYCODE_DEL,
        key_code::TAB => AKEYCODE_TAB,
        key_code::ENTER => AKEYCODE_ENTER,
        key_code::LEFT_SHIFT => AKEYCODE_SHIFT_LEFT,
        key_code::RIGHT_SHIFT => AKEYCODE_SHIFT_RIGHT,
        key_code::LEFT_CTRL => AKEYCODE_CTRL_LEFT,
        key_code::RIGHT_CTRL => AKEYCODE_CTRL_RIGHT,
        key_code::PAUSE => AKEYCODE_BREAK,
        key_code::CAPS_LOCK => AKEYCODE_CAPS_LOCK,
        key_code::ESCAPE => AKEYCODE_ESCAPE,
        key_code::SPACE => AKEYCODE_SPACE,
        key_code::PAGE_UP => AKEYCODE_PAGE_UP,
        key_code::PAGE_DOWN => AKEYCODE_PAGE_DOWN,
        key_code::END => AKEYCODE_MOVE_END,
        key_code::HOME => AKEYCODE_MOVE_HOME,
        key_code::LEFT => AKEYCODE_DPAD_LEFT,
        key_code::UP => AKEYCODE_DPAD_UP,
        key_code::RIGHT => AKEYCODE_DPAD_RIGHT,
        key_code::DOWN => AKEYCODE_DPAD_DOWN,
        key_code::INSERT => AKEYCODE_INSERT,
        key_code::DELETE => AKEYCODE_FORWARD_DEL,
        key_code::NUM_LOCK => AKEYCODE_NUM_LOCK,
        key_code::SCROLL_LOCK => AKEYCODE_SCROLL_LOCK,
        key_code::SEMICOLON => AKEYCODE_SEMICOLON,
        key_code::EQUAL => AKEYCODE_EQUALS,
        key_code::COMMA => AKEYCODE_COMMA,
        key_code::MINUS => AKEYCODE_MINUS,
        key_code::NUMPAD_ADD => AKEYCODE_NUMPAD_ADD,
        key_code::NUMPAD_SUBTRACT => AKEYCODE_NUMPAD_SUBTRACT,
        key_code::NUMPAD_MULTIPLY => AKEYCODE_NUMPAD_MULTIPLY,
        key_code::NUMPAD_DIVIDE => AKEYCODE_NUMPAD_DIVIDE,
        key_code::PERIOD => AKEYCODE_PERIOD,
        key_code::NUMPAD_DECIMAL => AKEYCODE_NUMPAD_DOT,
        key_code::SLASH => AKEYCODE_SLASH,
        key_code::GRAVE => AKEYCODE_GRAVE,
        key_code::LEFT_BRACKET => AKEYCODE_LEFT_BRACKET,
        key_code::BACKSLASH => AKEYCODE_BACKSLASH,
        key_code::RIGHT_BRACKET => AKEYCODE_RIGHT_BRACKET,
        key_code::APOSTROPHE => AKEYCODE_APOSTROPHE,
        key_code::MENU => AKEYCODE_MENU,
        key_code::LEFT_SUPER => AKEYCODE_META_LEFT,
        key_code::RIGHT_SUPER => AKEYCODE_META_RIGHT,
        key_code::LEFT_ALT => AKEYCODE_ALT_LEFT,
        key_code::RIGHT_ALT => AKEYCODE_ALT_RIGHT,
        _ => AKEYCODE_UNKNOWN,
    }
}

/// Matches GamepadButtonId enum values from game_window.h
#[no_mangle]
pub extern "C" fn window_callbacks_map_gamepad_key(btn: i32) -> i32 {
    match btn {
        0 => AKEYCODE_BUTTON_A,
        1 => AKEYCODE_BUTTON_B,
        2 => AKEYCODE_BUTTON_X,
        3 => AKEYCODE_BUTTON_Y,
        4 => AKEYCODE_BUTTON_L1,
        5 => AKEYCODE_BUTTON_R1,
        6 => AKEYCODE_BUTTON_SELECT,
        7 => AKEYCODE_BUTTON_START,
        8 => AKEYCODE_BUTTON_MODE,
        9 => AKEYCODE_BUTTON_THUMBL,
        10 => AKEYCODE_BUTTON_THUMBR,
        11 => AKEYCODE_DPAD_UP,
        12 => AKEYCODE_DPAD_RIGHT,
        13 => AKEYCODE_DPAD_DOWN,
        14 => AKEYCODE_DPAD_LEFT,
        _ => -1,
    }
}

// === FakeEGL: EGL stub library (ported from fake_egl.cpp) ===

const EGL_TRUE: i32 = 1;
const EGL_FALSE: i32 = 0;
const EGL_SUCCESS: i32 = 0x3000;
const EGL_NO_SURFACE: *mut c_void = std::ptr::null_mut();
const EGL_NO_CONTEXT: *mut c_void = std::ptr::null_mut();
const EGL_DRAW: i32 = 0x3059;
const EGL_WIDTH: i32 = 0x3057;
const EGL_HEIGHT: i32 = 0x3056;
const EGL_CONFIG_ID: i32 = 0x3028;
const EGL_NONE: i32 = 0x3038;
const EGL_VENDOR: i32 = 0x3053;
const EGL_VERSION: i32 = 0x3054;
const EGL_EXTENSIONS: i32 = 0x3055;
const EGL_NATIVE_VISUAL_ID: i32 = 0x302E;
const EGL_CONTEXT_CLIENT_VERSION: i32 = 0x3098;
// GL constants
const GL_VIEWPORT: u32 = 0x0BA2;
const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
const GL_RGBA: u32 = 0x1908;
const GL_UNSIGNED_BYTE: u32 = 0x1401;

pub mod fake_egl {

use super::*;
use std::thread::ThreadId;

#[repr(transparent)]
struct SendPtr<T>(*mut T);
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

// === State ===

pub static HOST_PROC_ADDR_FN: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static HOST_PROC_OVERRIDES: LazyLock<Mutex<HashMap<String, SendPtr<c_void>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub static SAVED_EGL_DISPLAY: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static SAVED_EGL_SURFACE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static SAVED_EGL_CONTEXT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static SAVED_EGL_CONFIG: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static SAVED_NATIVE_WINDOW: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

// Real EGL function pointers
pub static REAL_EGL_MAKE_CURRENT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_GET_ERROR: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_GET_CURRENT_DISPLAY: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_GET_CURRENT_SURFACE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_GET_CURRENT_CONTEXT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_CREATE_CONTEXT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_CHOOSE_CONFIG: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_GET_CONFIG_ATTRIB: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_CREATE_PBUFFER_SURFACE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_CREATE_WINDOW_SURFACE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_DESTROY_CONTEXT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_DESTROY_SURFACE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_QUERY_CONTEXT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_EGL_SWAP_BUFFERS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

// Real GL function pointers (debug wrappers)
pub static REAL_GL_GET_INTEGERV: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_GL_READ_PIXELS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_GL_GET_ERROR: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_GL_CLEAR_COLOR: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_GL_CLEAR: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_GL_DRAW_ARRAYS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_GL_DRAW_ELEMENTS: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_GL_BIND_FRAMEBUFFER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
pub static REAL_GL_USE_PROGRAM: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());



// Per-thread context tracking
pub static THREAD_CONTEXTS: LazyLock<Mutex<HashMap<ThreadId, SendPtr<c_void>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
pub static THREAD_SURFACES: LazyLock<Mutex<HashMap<ThreadId, SendPtr<c_void>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

std::thread_local! {
    pub static TLS_REAL_SURFACE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
}

// Swap buffers callbacks
#[repr(C)]
pub struct SwapBuffersCallback {
    user: SendPtr<c_void>,
    callback: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void)>,
}

pub static SWAP_BUFFERS_CALLBACKS: LazyLock<Mutex<Vec<SwapBuffersCallback>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));
pub static CURRENT_DRAW_SURFACE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

fn init_gl_debug_functions() {
    if !REAL_GL_GET_INTEGERV.load(Ordering::SeqCst).is_null() {
        return;
    }
    unsafe {
        // Must call the REAL resolver directly, NOT resolve_gl() which checks
        // the overrides map. The debug wrappers are registered as overrides, so
        // resolve_gl("glClearColor") would return the debug wrapper itself,
        // causing infinite recursion → stack overflow → SIGSEGV.
        let resolver_ptr = HOST_PROC_ADDR_FN.load(Ordering::SeqCst);
        if resolver_ptr.is_null() {
            return;
        }
        let resolver: extern "C" fn(*const c_char) -> *mut c_void =
            std::mem::transmute(resolver_ptr);
        REAL_GL_GET_INTEGERV.store(resolver(c"glGetIntegerv".as_ptr()), Ordering::SeqCst);
        REAL_GL_READ_PIXELS.store(resolver(c"glReadPixels".as_ptr()), Ordering::SeqCst);
        REAL_GL_GET_ERROR.store(resolver(c"glGetError".as_ptr()), Ordering::SeqCst);
        REAL_GL_CLEAR_COLOR.store(resolver(c"glClearColor".as_ptr()), Ordering::SeqCst);
        REAL_GL_CLEAR.store(resolver(c"glClear".as_ptr()), Ordering::SeqCst);
        REAL_GL_DRAW_ARRAYS.store(resolver(c"glDrawArrays".as_ptr()), Ordering::SeqCst);
        REAL_GL_DRAW_ELEMENTS.store(resolver(c"glDrawElements".as_ptr()), Ordering::SeqCst);
        REAL_GL_BIND_FRAMEBUFFER.store(resolver(c"glBindFramebuffer".as_ptr()), Ordering::SeqCst);
        REAL_GL_USE_PROGRAM.store(resolver(c"glUseProgram".as_ptr()), Ordering::SeqCst);
    }
}

// === GL debug wrappers ===

#[no_mangle]
pub unsafe extern "C" fn fake_egl_debug_clear_color(red: f32, green: f32, blue: f32, alpha: f32) {
    init_gl_debug_functions();
    if let Some(f) = REAL_GL_CLEAR_COLOR.load(Ordering::SeqCst).as_ref() {
        let func: extern "C" fn(f32, f32, f32, f32) = std::mem::transmute(f);
        func(red, green, blue, alpha);
    }
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_debug_clear(mask: u32) {
    init_gl_debug_functions();
    if let Some(f) = REAL_GL_CLEAR.load(Ordering::SeqCst).as_ref() {
        let func: extern "C" fn(u32) = std::mem::transmute(f);
        func(mask);
    }
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_debug_draw_arrays(mode: u32, first: i32, count: i32) {
    init_gl_debug_functions();
    if let Some(f) = REAL_GL_DRAW_ARRAYS.load(Ordering::SeqCst).as_ref() {
        let func: extern "C" fn(u32, i32, i32) = std::mem::transmute(f);
        func(mode, first, count);
    }
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_debug_draw_elements(
    mode: u32, count: i32, typ: u32, indices: *const c_void,
) {
    init_gl_debug_functions();
    if let Some(f) = REAL_GL_DRAW_ELEMENTS.load(Ordering::SeqCst).as_ref() {
        let func: extern "C" fn(u32, i32, u32, *const c_void) = std::mem::transmute(f);
        func(mode, count, typ, indices);
    }
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_debug_bind_framebuffer(target: u32, framebuffer: u32) {
    init_gl_debug_functions();
    if let Some(f) = REAL_GL_BIND_FRAMEBUFFER.load(Ordering::SeqCst).as_ref() {
        let func: extern "C" fn(u32, u32) = std::mem::transmute(f);
        func(target, framebuffer);
    }
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_debug_use_program(program: u32) {
    init_gl_debug_functions();
    if let Some(f) = REAL_GL_USE_PROGRAM.load(Ordering::SeqCst).as_ref() {
        let func: extern "C" fn(u32) = std::mem::transmute(f);
        func(program);
    }
}

// === EGL function implementations ===

#[no_mangle]
pub extern "C" fn fake_egl_initialize(_display: *mut c_void, major: *mut i32, minor: *mut i32) -> i32 {
    unsafe {
        if !major.is_null() { *major = 1; }
        if !minor.is_null() { *minor = 5; }
    }
    EGL_TRUE
}

#[no_mangle]
pub extern "C" fn fake_egl_terminate(_display: *mut c_void) -> i32 {
    EGL_TRUE
}

#[no_mangle]
pub extern "C" fn fake_egl_get_error() -> i32 {
    EGL_SUCCESS
}

#[no_mangle]
pub extern "C" fn fake_egl_query_string(_display: *mut c_void, name: i32) -> *const c_char {
    match name {
        EGL_VENDOR => c"mcpelauncher".as_ptr(),
        EGL_VERSION => c"1.5 mcpelauncher".as_ptr(),
        EGL_EXTENSIONS => c"".as_ptr(),
        _ => {
            log::warn!("[FakeEGL] eglQueryString {:x}", name);
            std::ptr::null()
        }
    }
}

#[no_mangle]
pub extern "C" fn fake_egl_get_display(_native: *mut c_void) -> *mut c_void {
    1 as *mut c_void
}

#[no_mangle]
pub extern "C" fn fake_egl_get_current_display() -> *mut c_void {
    1 as *mut c_void
}

#[no_mangle]
pub extern "C" fn fake_egl_get_current_context() -> *mut c_void {
    let surface = CURRENT_DRAW_SURFACE.load(Ordering::SeqCst);
    if surface.is_null() { std::ptr::null_mut() } else { 1 as *mut c_void }
}

#[no_mangle]
pub extern "C" fn fake_egl_choose_config(
    _display: *mut c_void, _attrib_list: *const i32, _configs: *mut c_void,
    _config_size: i32, num_config: *mut i32,
) -> i32 {
    unsafe { if !num_config.is_null() { *num_config = 1; } }
    EGL_TRUE
}

#[no_mangle]
pub extern "C" fn fake_egl_get_config_attrib(
    _display: *mut c_void, _config: *mut c_void, attribute: i32, value: *mut i32,
) -> i32 {
    match attribute {
        EGL_NATIVE_VISUAL_ID => unsafe { *value = 0; EGL_TRUE },
        n if n == 0x3024 || n == 0x3023 || n == 0x3022 || n == 0x3021 || n == 0x3025 || n == 0x3026 => {
            // EGL_RED_SIZE, GREEN_SIZE, BLUE_SIZE, ALPHA_SIZE, DEPTH_SIZE, STENCIL_SIZE
            unsafe { *value = 8; }
            EGL_TRUE
        }
        _ => {
            log::warn!("[FakeEGL] eglGetConfigAttrib {:x}", attribute);
            EGL_TRUE
        }
    }
}

#[no_mangle]
pub extern "C" fn fake_egl_create_window_surface(
    _display: *mut c_void, _config: *mut c_void, native_window: *mut c_void, _attrib_list: *const i32,
) -> *mut c_void {
    native_window
}

#[no_mangle]
pub extern "C" fn fake_egl_destroy_surface(_display: *mut c_void, _surface: *mut c_void) -> i32 {
    EGL_TRUE
}

#[no_mangle]
pub extern "C" fn fake_egl_create_context(
    _display: *mut c_void, _config: *mut c_void, _share_context: *mut c_void, _attrib_list: *const i32,
) -> *mut c_void {
    1 as *mut c_void
}

#[no_mangle]
pub extern "C" fn fake_egl_destroy_context(_display: *mut c_void, _context: *mut c_void) -> i32 {
    EGL_TRUE
}

fn get_or_create_thread_context() -> *mut c_void {
    unsafe {
        if REAL_EGL_CREATE_CONTEXT.load(Ordering::SeqCst).is_null()
            || SAVED_EGL_DISPLAY.load(Ordering::SeqCst).is_null()
            || SAVED_EGL_CONTEXT.load(Ordering::SeqCst).is_null()
        {
            return std::ptr::null_mut();
        }
        let tid = std::thread::current().id();
        {
            let contexts = THREAD_CONTEXTS.lock().unwrap();
            if let Some(ctx) = contexts.get(&tid) {
                return ctx.0;
            }
        }
        let create_ctx: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void =
            std::mem::transmute(REAL_EGL_CREATE_CONTEXT.load(Ordering::SeqCst));
        let ctx_attrs = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE, 0];
        let ctx = create_ctx(
            SAVED_EGL_DISPLAY.load(Ordering::SeqCst),
            SAVED_EGL_CONFIG.load(Ordering::SeqCst),
            SAVED_EGL_CONTEXT.load(Ordering::SeqCst),
            ctx_attrs.as_ptr(),
        );
        log::warn!("[FakeEGL]   created shared context for thread {:?}: {:p}", tid, ctx);
        if !ctx.is_null() {
            THREAD_CONTEXTS.lock().unwrap().insert(tid, SendPtr(ctx));
        }

        // Also create a per-thread window surface. Mesa's X11 EGL backend
        // doesn't allow binding a surface created on one thread to a context
        // on another thread — so we must create the surface on this thread too.
        if !ctx.is_null() {
            let create_surf_ptr = REAL_EGL_CREATE_WINDOW_SURFACE.load(Ordering::SeqCst);
            let native_win = SAVED_NATIVE_WINDOW.load(Ordering::SeqCst);
            let saved_disp = SAVED_EGL_DISPLAY.load(Ordering::SeqCst);
            let saved_cfg = SAVED_EGL_CONFIG.load(Ordering::SeqCst);
            log::warn!("[FakeEGL]   debug per-thread surf: create_surf={:p} native={:p} disp={:p} cfg={:p}",
                       create_surf_ptr, native_win, saved_disp, saved_cfg);
            if let Some(f) = create_surf_ptr.as_ref() {
                if !native_win.is_null() && !saved_disp.is_null() && !saved_cfg.is_null() {
                    let create_surf: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void =
                        std::mem::transmute(f);
                    let attribs = [EGL_NONE, 0];
                    let new_surf = create_surf(
                        saved_disp, saved_cfg, native_win, attribs.as_ptr(),
                    );
                    let egl_err_val = if let Some(err_f) = REAL_EGL_GET_ERROR.load(Ordering::SeqCst).as_ref() {
                        let err_fn: extern "C" fn() -> i32 = std::mem::transmute(err_f);
                        err_fn()
                    } else { -1 };
                    log::warn!("[FakeEGL]   created per-thread surface attempt result: {:p} err=0x{:x}", new_surf, egl_err_val);
                    if !new_surf.is_null() {
                        log::warn!("[FakeEGL]   created per-thread surface for thread {:?}: {:p}", tid, new_surf);
                        THREAD_SURFACES.lock().unwrap().insert(tid, SendPtr(new_surf));
                    }
                }
            }
        }
        ctx
    }
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_make_current(
    display: *mut c_void, draw: *mut c_void, read: *mut c_void, context: *mut c_void,
) -> i32 {
    log::warn!("[FakeEGL] eglMakeCurrent display={:p} draw={:p} read={:p} context={:p} tid={:?}",
               display, draw, read, context, std::thread::current().id());

    if !draw.is_null() {
        let mut target_ctx = SAVED_EGL_CONTEXT.load(Ordering::SeqCst);
        let mut target_surface = SAVED_EGL_SURFACE.load(Ordering::SeqCst);
        let mut use_direct = true;

        let tid = std::thread::current().id();
        {
            let contexts = THREAD_CONTEXTS.lock().unwrap();
            if let Some(ctx) = contexts.get(&tid) {
                target_ctx = ctx.0;
                use_direct = false;
            }
            if let Some(surf) = THREAD_SURFACES.lock().unwrap().get(&tid) {
                target_surface = surf.0;
            }
        }

        let real_make_current: Option<extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> i32> =
            REAL_EGL_MAKE_CURRENT.load(Ordering::SeqCst).as_ref().map(|p| std::mem::transmute(p));

        // If no saved handles, use eglut's handles directly
        if target_ctx.is_null() {
            unsafe {
                let egl_dpy = crate::eglut::state::STATE.egl_dpy;
                let win_ref = &*std::ptr::addr_of!(crate::eglut::state::STATE.current_window);
                if let Some(eglut_win) = win_ref.as_ref() {
                    target_ctx = eglut_win.context;
                    target_surface = eglut_win.surface;
                    // Populate ALL saved handles from eglut state.
                    // The Rust eglut does NOT call eglMakeCurrent in eglutCreateWindow,
                    // so saveCurrentWindowHandle gets NULL for all handles.
                    // This is needed so that:
                    // 1) real_eglMakeCurrent can be called (needs display + surface + context)
                    // 2) get_or_create_thread_context can create a shared context
                    //    if direct real_eglMakeCurrent fails (EGL_BAD_ACCESS from Mesa
                    //    X11 thread affinity — context created on main thread but used
                    //    on game thread).
                    SAVED_EGL_DISPLAY.store(egl_dpy as *mut c_void, Ordering::SeqCst);
                    SAVED_EGL_SURFACE.store(eglut_win.surface as *mut c_void, Ordering::SeqCst);
                    SAVED_EGL_CONTEXT.store(eglut_win.context as *mut c_void, Ordering::SeqCst);
                    SAVED_EGL_CONFIG.store(eglut_win.config as *mut c_void, Ordering::SeqCst);
                    log::warn!("[FakeEGL]   fallback to eglut handles: dpy={:p} surf={:p} ctx={:p}",
                               egl_dpy, eglut_win.surface, eglut_win.context);
                }
            }
        }

        // If no primary context exists yet, create EGL context + surface on THIS
        // thread. Mesa's X11 EGL backend has thread affinity — objects created on
        // one thread cannot be made current on another thread (EGL_BAD_ACCESS).
        // Deferring creation to the game/render thread avoids this entirely.
        if target_ctx.is_null() {
            let saved_disp = SAVED_EGL_DISPLAY.load(Ordering::SeqCst);
            let saved_cfg = SAVED_EGL_CONFIG.load(Ordering::SeqCst);
            let native_win = SAVED_NATIVE_WINDOW.load(Ordering::SeqCst);
            if !saved_disp.is_null() && !saved_cfg.is_null() && !native_win.is_null() {
                if let Some(create_ctx_f) = REAL_EGL_CREATE_CONTEXT.load(Ordering::SeqCst).as_ref() {
                    let create_ctx_fn: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void =
                        std::mem::transmute(create_ctx_f);
                    let ctx_attrs = [EGL_CONTEXT_CLIENT_VERSION, 2, EGL_NONE, 0];
                    let new_ctx = create_ctx_fn(saved_disp, saved_cfg, std::ptr::null_mut(), ctx_attrs.as_ptr());
                    if !new_ctx.is_null() {
                        if let Some(create_surf_f) = REAL_EGL_CREATE_WINDOW_SURFACE.load(Ordering::SeqCst).as_ref() {
                            let create_surf_fn: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *const i32) -> *mut c_void =
                                std::mem::transmute(create_surf_f);
                            let surf_attribs = [EGL_NONE, 0];
                            let new_surf = create_surf_fn(saved_disp, saved_cfg, native_win, surf_attribs.as_ptr());
                            if !new_surf.is_null() {
                                log::warn!("[FakeEGL]   created primary context+surface on thread {:?}: ctx={:p} surf={:p}",
                                           tid, new_ctx, new_surf);
                                target_ctx = new_ctx;
                                target_surface = new_surf;
                                use_direct = true;
                                SAVED_EGL_CONTEXT.store(new_ctx, Ordering::SeqCst);
                                SAVED_EGL_SURFACE.store(new_surf, Ordering::SeqCst);
                                THREAD_CONTEXTS.lock().unwrap().insert(tid, SendPtr(new_ctx));
                                THREAD_SURFACES.lock().unwrap().insert(tid, SendPtr(new_surf));
                            } else {
                                log::warn!("[FakeEGL]   failed to create primary surface on thread {:?}", tid);
                            }
                        }
                    } else {
                        log::warn!("[FakeEGL]   failed to create primary context on thread {:?}", tid);
                    }
                }
            }
        }

        if let Some(make_cur) = real_make_current {
            let saved_disp = SAVED_EGL_DISPLAY.load(Ordering::SeqCst);
            if !target_ctx.is_null() && !saved_disp.is_null() {
                if !use_direct {
                    log::warn!("[FakeEGL]   using per-thread shared ctx={:p} surf={:p}", target_ctx, target_surface);
                }
                log::warn!("[FakeEGL]   calling real_eglMakeCurrent(disp={:p}, surf={:p}, ctx={:p})",
                           saved_disp, target_surface, target_ctx);
                let ok = make_cur(saved_disp, target_surface, target_surface, target_ctx);
                if ok != 0 {
                    CURRENT_DRAW_SURFACE.store(draw, Ordering::SeqCst);
                    TLS_REAL_SURFACE.with(|s| s.store(target_surface, Ordering::SeqCst));
                    log::warn!("[FakeEGL]   real_eglMakeCurrent SUCCEEDED, tls_real_surface={:p}", target_surface);
                    return EGL_TRUE;
                }
                let egl_err = if let Some(f) = REAL_EGL_GET_ERROR.load(Ordering::SeqCst).as_ref() {
                    let func: extern "C" fn() -> i32 = std::mem::transmute(f);
                    func()
                } else { -1 };
                log::warn!("[FakeEGL]   real eglMakeCurrent failed, err=0x{:x}, creating shared context", egl_err);
                let new_ctx = get_or_create_thread_context();
                if !new_ctx.is_null() && new_ctx != target_ctx {
                    // Use per-thread surface if one was just created by get_or_create_thread_context
                    if let Some(surf) = THREAD_SURFACES.lock().unwrap().get(&tid) {
                        target_surface = surf.0;
                    }
                    let ok2 = make_cur(saved_disp, target_surface, target_surface, new_ctx);
                    if ok2 != 0 {
                        log::warn!("[FakeEGL]   made shared context current on this thread, surf={:p}", target_surface);
                        CURRENT_DRAW_SURFACE.store(draw, Ordering::SeqCst);
                        TLS_REAL_SURFACE.with(|s| s.store(target_surface, Ordering::SeqCst));
                        return EGL_TRUE;
                    }
                    let err2 = if let Some(f) = REAL_EGL_GET_ERROR.load(Ordering::SeqCst).as_ref() {
                        let func: extern "C" fn() -> i32 = std::mem::transmute(f);
                        func()
                    } else { -1 };
                    log::warn!("[FakeEGL]   shared context eglMakeCurrent also failed, err=0x{:x}", err2);
                }
            }
        }
        log::warn!("[FakeEGL]   could not make context current, returning EGL_TRUE anyway");
        CURRENT_DRAW_SURFACE.store(draw, Ordering::SeqCst);
        EGL_TRUE
    } else {
        if let Some(f) = REAL_EGL_MAKE_CURRENT.load(Ordering::SeqCst).as_ref() {
            let saved_disp = SAVED_EGL_DISPLAY.load(Ordering::SeqCst);
            if !saved_disp.is_null() {
                let make_cur: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> i32 =
                    std::mem::transmute(f);
                make_cur(saved_disp, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
            }
        }
        CURRENT_DRAW_SURFACE.store(std::ptr::null_mut(), Ordering::SeqCst);
        TLS_REAL_SURFACE.with(|s| s.store(std::ptr::null_mut(), Ordering::SeqCst));
        EGL_TRUE
    }
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_swap_buffers(display: *mut c_void, surface: *mut c_void) -> i32 {
    {
        let cbs = SWAP_BUFFERS_CALLBACKS.lock().unwrap();
        for cb in cbs.iter() {
            if let Some(func) = cb.callback {
                func(cb.user.0, display, surface);
            }
        }
    }

    let real_swap = REAL_EGL_SWAP_BUFFERS.load(Ordering::SeqCst);
    if !real_swap.is_null() {
        let saved_disp = SAVED_EGL_DISPLAY.load(Ordering::SeqCst);
        let tls_surf = TLS_REAL_SURFACE.with(|s| s.load(Ordering::SeqCst));
        if !saved_disp.is_null() && !tls_surf.is_null() {
            let swap_fn: extern "C" fn(*mut c_void, *mut c_void) -> i32 = std::mem::transmute(real_swap);
            let ok = swap_fn(saved_disp, tls_surf);
            if ok == 0 {
                let egl_err = if let Some(f) = REAL_EGL_GET_ERROR.load(Ordering::SeqCst).as_ref() {
                    let func: extern "C" fn() -> i32 = std::mem::transmute(f);
                    func()
                } else { -1 };
                log::warn!("[FakeEGL] eglSwapBuffers FAILED: display={:p} tls_surf={:p} err=0x{:x} tid={:?}",
                           saved_disp, tls_surf, egl_err, std::thread::current().id());
            }
            return ok;
        }
    }

    log::warn!("[FakeEGL] eglSwapBuffers display={:p} surface={:p} (fallback) tid={:?}",
               display, surface, std::thread::current().id());
    let saved_disp = SAVED_EGL_DISPLAY.load(Ordering::SeqCst);
    let saved_surf = SAVED_EGL_SURFACE.load(Ordering::SeqCst);
    if !saved_disp.is_null() && !saved_surf.is_null() {
        if let Some(f) = REAL_EGL_SWAP_BUFFERS.load(Ordering::SeqCst).as_ref() {
            let swap_fn: extern "C" fn(*mut c_void, *mut c_void) -> i32 = std::mem::transmute(f);
            return swap_fn(saved_disp, saved_surf);
        }
    }
    // If no saved handles, try eglut's display and surface directly
    unsafe {
        let egl_dpy = crate::eglut::state::STATE.egl_dpy;
        let win_ref = &*std::ptr::addr_of!(crate::eglut::state::STATE.current_window);
        if let Some(eglut_win) = win_ref.as_ref() {
            if !egl_dpy.is_null() && !eglut_win.surface.is_null() {
                if let Some(f) = REAL_EGL_SWAP_BUFFERS.load(Ordering::SeqCst).as_ref() {
                    let swap_fn: extern "C" fn(*mut c_void, *mut c_void) -> i32 = std::mem::transmute(f);
                    log::warn!("[FakeEGL]   using eglut display/surface: dpy={:p} surf={:p}",
                               egl_dpy, eglut_win.surface);
                    return swap_fn(egl_dpy as *mut c_void, eglut_win.surface as *mut c_void);
                }
            }
        }
    }
    EGL_TRUE
}

#[no_mangle]
pub extern "C" fn fake_egl_swap_interval(_display: *mut c_void, _interval: i32) -> i32 {
    EGL_TRUE
}

#[no_mangle]
pub extern "C" fn fake_egl_query_surface(
    _display: *mut c_void, _surface: *mut c_void, attribute: i32, value: *mut i32,
) -> i32 {
    if attribute == EGL_WIDTH || attribute == EGL_HEIGHT {
        let saved_win = SAVED_NATIVE_WINDOW.load(Ordering::SeqCst);
        if !saved_win.is_null() {
            unsafe {
                extern "C" {
                    fn eglutGetDisplay() -> *mut c_void;
                    fn eglutGetWindowHandle() -> u64;
                }
                let dpy = eglutGetDisplay();
                if !dpy.is_null() {
                    let display: *mut c_void = dpy;
                    let win = saved_win;
                    let mut root: u64 = 0;
                    let mut x: i32 = 0;
                    let mut y: i32 = 0;
                    let mut w: u32 = 0;
                    let mut h: u32 = 0;
                    let mut border: u32 = 0;
                    let mut depth: u32 = 0;
                    extern "C" {
                        fn XGetGeometry(
                            display: *mut c_void, win: u64,
                            root_return: *mut u64,
                            x_return: *mut i32, y_return: *mut i32,
                            width_return: *mut u32, height_return: *mut u32,
                            border_width_return: *mut u32, depth_return: *mut u32,
                        ) -> i32;
                    }
                    if XGetGeometry(display, win as u64, &mut root, &mut x, &mut y, &mut w, &mut h, &mut border, &mut depth) != 0 {
                        *value = if attribute == EGL_WIDTH { w as i32 } else { h as i32 };
                        return EGL_TRUE;
                    }
                }
            }
        }
        unsafe { *value = 32; }
        return EGL_TRUE;
    }
    log::warn!("[FakeEGL] eglQuerySurface {:x}", attribute);
    EGL_TRUE
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_get_proc_address(name: *const c_char) -> *mut c_void {
    if name.is_null() { return std::ptr::null_mut(); }
    let name_str = CStr::from_ptr(name).to_str().unwrap_or("");
    let overrides = HOST_PROC_OVERRIDES.lock().unwrap();
    if let Some(ptr) = overrides.get(name_str) {
        return ptr.0;
    }
    let resolver = HOST_PROC_ADDR_FN.load(Ordering::SeqCst);
    if resolver.is_null() {
        return std::ptr::null_mut();
    }
    let func: extern "C" fn(*const c_char) -> *mut c_void = std::mem::transmute(resolver);
    func(name)
}

// === EGL wait client stub used by installLibrary ===

#[no_mangle]
pub extern "C" fn fake_egl_wait_client() -> i32 { EGL_TRUE }

// === FakeEGL class methods ===

#[no_mangle]
pub extern "C" fn fake_egl_set_proc_addr_function(fn_ptr: *mut c_void) {
    HOST_PROC_ADDR_FN.store(fn_ptr, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_install_library() {
    // Build name/function pointer arrays for linker::load_library
    let egl_names: &[*const c_char] = &[
        c"eglInitialize".as_ptr(),
        c"eglTerminate".as_ptr(),
        c"eglGetError".as_ptr(),
        c"eglQueryString".as_ptr(),
        c"eglGetDisplay".as_ptr(),
        c"eglGetCurrentDisplay".as_ptr(),
        c"eglGetCurrentContext".as_ptr(),
        c"eglChooseConfig".as_ptr(),
        c"eglGetConfigAttrib".as_ptr(),
        c"eglCreateWindowSurface".as_ptr(),
        c"eglDestroySurface".as_ptr(),
        c"eglCreateContext".as_ptr(),
        c"eglDestroyContext".as_ptr(),
        c"eglMakeCurrent".as_ptr(),
        c"eglSwapBuffers".as_ptr(),
        c"eglSwapInterval".as_ptr(),
        c"eglQuerySurface".as_ptr(),
        c"eglGetProcAddress".as_ptr(),
        c"eglWaitClient".as_ptr(),
    ];
    let egl_funcs: &[*mut c_void] = &[
        fake_egl_initialize as *mut c_void,
        fake_egl_terminate as *mut c_void,
        fake_egl_get_error as *mut c_void,
        fake_egl_query_string as *mut c_void,
        fake_egl_get_display as *mut c_void,
        fake_egl_get_current_display as *mut c_void,
        fake_egl_get_current_context as *mut c_void,
        fake_egl_choose_config as *mut c_void,
        fake_egl_get_config_attrib as *mut c_void,
        fake_egl_create_window_surface as *mut c_void,
        fake_egl_destroy_surface as *mut c_void,
        fake_egl_create_context as *mut c_void,
        fake_egl_destroy_context as *mut c_void,
        fake_egl_make_current as *mut c_void,
        fake_egl_swap_buffers as *mut c_void,
        fake_egl_swap_interval as *mut c_void,
        fake_egl_query_surface as *mut c_void,
        fake_egl_get_proc_address as *mut c_void,
        fake_egl_wait_client as *mut c_void,
    ];
    let count = egl_names.len() as i32;

    extern "C" {
        fn linker_load_library(
            name: *const c_char,
            names: *const *const c_char,
            funcs: *const *mut c_void,
            count: i32,
        );
    }
    linker_load_library(c"libEGL.so".as_ptr(), egl_names.as_ptr(), egl_funcs.as_ptr(), count);

    // Load real EGL functions via dlopen/dlsym
    let libegl = libc::dlopen(c"libEGL.so".as_ptr() as *const libc::c_char, libc::RTLD_LAZY | libc::RTLD_LOCAL);
    if !libegl.is_null() {
        macro_rules! dlsym_egl {
            ($var:expr, $name:literal) => {
                $var.store(libc::dlsym(libegl, concat!($name, "\0").as_ptr() as *const libc::c_char), Ordering::SeqCst);
            };
        }
        dlsym_egl!(REAL_EGL_MAKE_CURRENT, "eglMakeCurrent");
        dlsym_egl!(REAL_EGL_GET_ERROR, "eglGetError");
        dlsym_egl!(REAL_EGL_GET_CURRENT_DISPLAY, "eglGetCurrentDisplay");
        dlsym_egl!(REAL_EGL_GET_CURRENT_SURFACE, "eglGetCurrentSurface");
        dlsym_egl!(REAL_EGL_GET_CURRENT_CONTEXT, "eglGetCurrentContext");
        dlsym_egl!(REAL_EGL_CREATE_CONTEXT, "eglCreateContext");
        dlsym_egl!(REAL_EGL_CHOOSE_CONFIG, "eglChooseConfig");
        dlsym_egl!(REAL_EGL_GET_CONFIG_ATTRIB, "eglGetConfigAttrib");
        dlsym_egl!(REAL_EGL_CREATE_PBUFFER_SURFACE, "eglCreatePbufferSurface");
        dlsym_egl!(REAL_EGL_CREATE_WINDOW_SURFACE, "eglCreateWindowSurface");
        dlsym_egl!(REAL_EGL_DESTROY_CONTEXT, "eglDestroyContext");
        dlsym_egl!(REAL_EGL_DESTROY_SURFACE, "eglDestroySurface");
        dlsym_egl!(REAL_EGL_QUERY_CONTEXT, "eglQueryContext");
        dlsym_egl!(REAL_EGL_SWAP_BUFFERS, "eglSwapBuffers");
        log::info!("[FakeEGL] real EGL functions loaded: makeCurrent={:p} createContext={:p} swapBuffers={:p}",
                   REAL_EGL_MAKE_CURRENT.load(Ordering::SeqCst),
                   REAL_EGL_CREATE_CONTEXT.load(Ordering::SeqCst),
                   REAL_EGL_SWAP_BUFFERS.load(Ordering::SeqCst));
    }
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_setup_gl_overrides() {
    let mut overrides = HOST_PROC_OVERRIDES.lock().unwrap();

    // MESA 23.1 blackscreen workarounds
    overrides.insert("glDrawElementsInstancedOES".into(), SendPtr(std::ptr::null_mut()));
    overrides.insert("glDrawArraysInstancedOES".into(), SendPtr(std::ptr::null_mut()));
    overrides.insert("glVertexAttribDivisorOES".into(), SendPtr(std::ptr::null_mut()));
    // NVIDIA stub
    overrides.insert("glInvalidateFramebuffer".into(), SendPtr(fake_egl_wait_client as *mut c_void));

    // Debug wrappers
    overrides.insert("glClearColor".into(), SendPtr(fake_egl_debug_clear_color as *mut c_void));
    overrides.insert("glClear".into(), SendPtr(fake_egl_debug_clear as *mut c_void));
    overrides.insert("glDrawArrays".into(), SendPtr(fake_egl_debug_draw_arrays as *mut c_void));
    overrides.insert("glDrawElements".into(), SendPtr(fake_egl_debug_draw_elements as *mut c_void));
    overrides.insert("glBindFramebuffer".into(), SendPtr(fake_egl_debug_bind_framebuffer as *mut c_void));
    overrides.insert("glUseProgram".into(), SendPtr(fake_egl_debug_use_program as *mut c_void));

    drop(overrides);

    // GLCorePatch overrides (Rust)
    extern "C" {
        fn mc_glcorepatch_install_gl(
            resolver: unsafe extern "C" fn(*const c_char) -> *mut c_void,
            add_override: unsafe extern "C" fn(*const c_char, *mut c_void),
        );
    }
    mc_glcorepatch_install_gl(fake_egl_get_proc_address, fake_egl_add_override);
}

// Helper for GLCorePatch to register overrides
#[no_mangle]
pub unsafe extern "C" fn fake_egl_add_override(name: *const c_char, func: *mut c_void) {
    if name.is_null() { return; }
    let s = CStr::from_ptr(name).to_str().unwrap_or("");
    let mut overrides = HOST_PROC_OVERRIDES.lock().unwrap();
    overrides.insert(s.into(), SendPtr(func));
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_save_current_window_handle() {
    if let Some(f) = REAL_EGL_GET_CURRENT_DISPLAY.load(Ordering::SeqCst).as_ref() {
        let func: extern "C" fn() -> *mut c_void = std::mem::transmute(f);
        SAVED_EGL_DISPLAY.store(func(), Ordering::SeqCst);
    }
    if let Some(f) = REAL_EGL_GET_CURRENT_SURFACE.load(Ordering::SeqCst).as_ref() {
        let func: extern "C" fn(i32) -> *mut c_void = std::mem::transmute(f);
        SAVED_EGL_SURFACE.store(func(EGL_DRAW), Ordering::SeqCst);
    }
    if let Some(f) = REAL_EGL_GET_CURRENT_CONTEXT.load(Ordering::SeqCst).as_ref() {
        let func: extern "C" fn() -> *mut c_void = std::mem::transmute(f);
        SAVED_EGL_CONTEXT.store(func(), Ordering::SeqCst);
    }

    // Save the EGL config from the current context
    let query_ctx = REAL_EGL_QUERY_CONTEXT.load(Ordering::SeqCst);
    let choose_cfg = REAL_EGL_CHOOSE_CONFIG.load(Ordering::SeqCst);
    if !query_ctx.is_null() && !choose_cfg.is_null() {
        let saved_disp = SAVED_EGL_DISPLAY.load(Ordering::SeqCst);
        let saved_ctx = SAVED_EGL_CONTEXT.load(Ordering::SeqCst);
        if !saved_disp.is_null() && !saved_ctx.is_null() {
            let qc: extern "C" fn(*mut c_void, *mut c_void, i32, *mut i32) = std::mem::transmute(query_ctx);
            let cc: extern "C" fn(*mut c_void, *const i32, *mut c_void, i32, *mut i32) -> i32 = std::mem::transmute(choose_cfg);
            let mut config_id: i32 = 0;
            qc(saved_disp, saved_ctx, EGL_CONFIG_ID, &mut config_id);
            let attribs = [EGL_CONFIG_ID, config_id, EGL_NONE, 0];
            let mut cfg: *mut c_void = std::ptr::null_mut();
            let mut num: i32 = 0;
            cc(saved_disp, attribs.as_ptr(), &mut cfg as *mut *mut c_void as *mut c_void, 1, &mut num);
            SAVED_EGL_CONFIG.store(cfg, Ordering::SeqCst);
            log::info!("[FakeEGL] saved EGL config: config_id={} config={:p}", config_id, cfg);
        }
    }

    log::info!("[FakeEGL] saved EGL handles: display={:p} surface={:p} context={:p}",
               SAVED_EGL_DISPLAY.load(Ordering::SeqCst),
               SAVED_EGL_SURFACE.load(Ordering::SeqCst),
               SAVED_EGL_CONTEXT.load(Ordering::SeqCst));
}

#[no_mangle]
pub extern "C" fn fake_egl_save_native_window(window: u64) {
    SAVED_NATIVE_WINDOW.store(window as *mut c_void, Ordering::SeqCst);
    log::info!("[FakeEGL] saved native window: {:x}", window);
}

#[no_mangle]
pub unsafe extern "C" fn fake_egl_release_context() {
    let saved_disp = SAVED_EGL_DISPLAY.load(Ordering::SeqCst);
    if let Some(f) = REAL_EGL_MAKE_CURRENT.load(Ordering::SeqCst).as_ref() {
        if !saved_disp.is_null() {
            let func: extern "C" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> i32 = std::mem::transmute(f);
            func(saved_disp, EGL_NO_SURFACE, EGL_NO_SURFACE, EGL_NO_CONTEXT);
            log::info!("[FakeEGL] released GL context from Rust main thread");
        }
    }
}

#[no_mangle]
pub extern "C" fn fake_egl_add_swap_buffers_callback(
    user: *mut c_void,
    callback: Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut c_void)>,
) {
    let mut cbs = SWAP_BUFFERS_CALLBACKS.lock().unwrap();
    cbs.push(SwapBuffersCallback { user: SendPtr(user), callback });
}

} // mod fake_egl

// === JNI bridge orchestration (ported from jni_bridge.cpp) ===


// Pure Rust SHA256 implementation (no OpenSSL dependency needed)
struct Sha256Ctx {
    state: [u32; 8],
    count: u64,
    buf: [u8; 64],
}

const K256: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

fn sha256_compress(state: &mut [u32; 8], block: &[u8; 64]) {
    let mut w = [0u32; 64];
    for i in 0..16 {
        w[i] = u32::from_be_bytes([block[i*4], block[i*4+1], block[i*4+2], block[i*4+3]]);
    }
    for i in 16..64 {
        let s0 = w[i-15].rotate_right(7) ^ w[i-15].rotate_right(18) ^ (w[i-15] >> 3);
        let s1 = w[i-2].rotate_right(17) ^ w[i-2].rotate_right(19) ^ (w[i-2] >> 10);
        w[i] = w[i-16].wrapping_add(s0).wrapping_add(w[i-7]).wrapping_add(s1);
    }
    let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut h) =
        (state[0], state[1], state[2], state[3], state[4], state[5], state[6], state[7]);
    for i in 0..64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ (!e & g);
        let temp1 = h.wrapping_add(s1).wrapping_add(ch).wrapping_add(K256[i]).wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);
        h = g; g = f; f = e; e = d.wrapping_add(temp1);
        d = c; c = b; b = a; a = temp1.wrapping_add(temp2);
    }
    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
    state[5] = state[5].wrapping_add(f);
    state[6] = state[6].wrapping_add(g);
    state[7] = state[7].wrapping_add(h);
}

#[no_mangle]
pub unsafe extern "C" fn shahasher_init_rust() -> *mut c_void {
    let ctx = Box::new(Sha256Ctx {
        state: [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19],
        count: 0,
        buf: [0u8; 64],
    });
    Box::into_raw(ctx) as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn shahasher_add_bytes_rust(ctx: *mut c_void, data: *const u8, len: i32) {
    let ctx = &mut *(ctx as *mut Sha256Ctx);
    let data = std::slice::from_raw_parts(data, len as usize);
    let mut offset = 0;
    let buf_used = (ctx.count as usize) & 63;
    if buf_used > 0 {
        let space = 64 - buf_used;
        let take = data.len().min(space);
        ctx.buf[buf_used..buf_used + take].copy_from_slice(&data[..take]);
        offset += take;
        if buf_used + take == 64 {
            sha256_compress(&mut ctx.state, &ctx.buf);
        }
    }
    while offset + 64 <= data.len() {
        let block: &[u8; 64] = data[offset..offset + 64].try_into().unwrap();
        sha256_compress(&mut ctx.state, block);
        offset += 64;
    }
    if offset < data.len() {
        let remaining = data.len() - offset;
        ctx.buf[..remaining].copy_from_slice(&data[offset..]);
    }
    ctx.count = ctx.count.wrapping_add(len as u64);
}

#[no_mangle]
pub unsafe extern "C" fn shahasher_sign_hash_rust(ctx: *mut c_void, out_len: *mut i32) -> *mut u8 {
    let ctx = &mut *(ctx as *mut Sha256Ctx);
    let bit_count = ctx.count.wrapping_mul(8);
    let buf_used = (ctx.count as usize) & 63;
    ctx.buf[buf_used] = 0x80;
    if buf_used >= 56 {
        for i in buf_used + 1..64 { ctx.buf[i] = 0; }
        sha256_compress(&mut ctx.state, &ctx.buf);
        ctx.buf = [0u8; 64];
    } else {
        for i in buf_used + 1..56 { ctx.buf[i] = 0; }
    }
    ctx.buf[56..64].copy_from_slice(&bit_count.to_be_bytes());
    sha256_compress(&mut ctx.state, &ctx.buf);
    let mut hash = [0u8; 32];
    for i in 0..8 {
        hash[i*4..i*4+4].copy_from_slice(&ctx.state[i].to_be_bytes());
    }
    *out_len = 32;
    let result = hash.to_vec().into_boxed_slice();
    Box::into_raw(result) as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn shahasher_free_rust(ctx: *mut c_void) {
    drop(Box::from_raw(ctx as *mut Sha256Ctx));
}

#[no_mangle]
pub unsafe extern "C" fn securerandom_generate_bytes_rust(bytes: i32, out_len: *mut i32) -> *mut u8 {
    use std::io::Read;
    let mut file = match std::fs::File::open("/dev/urandom") {
        Ok(f) => f,
        Err(_) => {
            *out_len = 0;
            return std::ptr::null_mut();
        }
    };
    let mut buf = vec![0u8; bytes as usize];
    if file.read_exact(&mut buf).is_err() {
        *out_len = 0;
        return std::ptr::null_mut();
    }
    let result = buf.into_boxed_slice();
    *out_len = result.len() as i32;
    Box::into_raw(result) as *mut u8
}

// === JNI class implementations (ported from src/jni/*.cpp) ===

// Base64 decode tables
static B64_DECODE: [u8; 256] = {
    const fn init() -> [u8; 256] {
        let mut t = [0xffu8; 256];
        let chars = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut i = 0;
        while i < chars.len() {
            t[chars[i] as usize] = i as u8;
            i += 1;
        }
        t[b'=' as usize] = 0;
        t
    }
    init()
};

#[no_mangle]
pub unsafe extern "C" fn jbase64_decode_rust(data: *const c_char, len: i32, out_len: *mut i32) -> *mut u8 {
    if data.is_null() || len <= 0 {
        *out_len = 0;
        return std::ptr::null_mut();
    }
    let slice = std::slice::from_raw_parts(data as *const u8, len as usize);
    // Strip whitespace/newlines
    let clean: Vec<u8> = slice.iter().copied().filter(|&b| b != b'\r' && b != b'\n').collect();
    if clean.is_empty() {
        *out_len = 0;
        return std::ptr::null_mut();
    }
    let approx = clean.len() * 3 / 4;
    let mut out = Vec::with_capacity(approx);
    let mut buf: u32 = 0;
    let mut bits = 0;
    for &b in &clean {
        let val = B64_DECODE[b as usize];
        if val == 0xff {
            continue;
        }
        buf = (buf << 6) | val as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    let result = out.into_boxed_slice();
    *out_len = result.len() as i32;
    Box::into_raw(result) as *mut u8
}

#[no_mangle]
pub unsafe extern "C" fn base64_encode_rust(
    data: *const u8,
    len: i32,
    padded: i32,
) -> *mut c_char {
    if data.is_null() || len <= 0 {
        return std::ptr::null_mut();
    }
    let slice = std::slice::from_raw_parts(data, len as usize);
    let encoded = util::base64::encode(slice, padded != 0);
    CString::new(encoded).unwrap_or_default().into_raw()
}

#[no_mangle]
pub unsafe extern "C" fn file_util_read_file_rust(
    path: *const c_char,
    out_len: *mut i32,
) -> *mut u8 {
    if path.is_null() {
        if !out_len.is_null() { *out_len = 0; }
        return std::ptr::null_mut();
    }
    let p = CStr::from_ptr(path).to_string_lossy().into_owned();
    match std::fs::read(&p) {
        Ok(bytes) => {
            let len = bytes.len() as i32;
            let boxed = bytes.into_boxed_slice();
            *out_len = len;
            Box::into_raw(boxed) as *mut u8
        }
        Err(e) => {
            log::debug!("file_util_read_file_rust: failed to read '{}': {}", p, e);
            *out_len = 0;
            std::ptr::null_mut()
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn arrays_copy_of_range_rust(
    data: *const c_void,
    offset: i32,
    len: i32,
    out_len: *mut i32,
) -> *mut u8 {
    let ptr = (data as *const u8).add(offset as usize);
    let slice = std::slice::from_raw_parts(ptr, len as usize);
    let result = slice.to_vec().into_boxed_slice();
    *out_len = result.len() as i32;
    Box::into_raw(result) as *mut u8
}

// ============================================================
// JNI native symbol resolver for Rust JniSupport
// ============================================================

static JNI_GAME_HANDLE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

#[no_mangle]
pub extern "C" fn jni_set_game_handle(handle: *mut c_void) {
    JNI_GAME_HANDLE.store(handle, Ordering::SeqCst);
}

#[no_mangle]
pub unsafe extern "C" fn jni_resolve_symbol(sym: *const c_char) -> *mut c_void {
    // First try the host process (our own binary) — this finds native methods
    // defined in C++ files like main_activity.cpp, xbox_live.cpp, etc.
    let host = libc::dlsym(std::ptr::null_mut(), sym);
    if !host.is_null() {
        return host;
    }
    // Then try the game library (libminecraftpe.so)
    let handle = JNI_GAME_HANDLE.load(Ordering::SeqCst);
    if handle.is_null() { return std::ptr::null_mut(); }
    extern "C" {
        fn mc_dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    }
    mc_dlsym(handle, sym)
}

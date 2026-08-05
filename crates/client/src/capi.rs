//! C bridge to the remaining compiled C++ mcpelauncher code.
//!
//! Phase 10: the original `capi.cpp` bridge (static lib `mcpelauncher-client-bridge`)
//! has been deleted. Its four functions were ported into this module:
//! `setup_paths`, `get_libc_symbols_from_cpp`, `load_core_libraries` and the
//! C-facing `mc_relocate_glesv2_symbols` (still called from `jni_bridge_stub.cpp`).
//!
//! The externs below are still defined in C++ (`jni_bridge_stub.cpp`,
//! `android_log_varargs.cpp`, `fake_assetmanager_stub.cpp`, …).

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CString};

use minecraft_imported_symbols::GLESV2_SYMBOLS;

extern "C" {
    fn mc_setup_android_hooks();
    fn mc_create_window_and_setup_graphics();
    pub fn mc_egl_swap_buffers(display: *mut c_void, surface: *mut c_void) -> i32;
    fn mc_dlsym(handle: *mut c_void, symbol: *const i8) -> *mut c_void;
    fn jni_support_register_minecraft_natives_cpp(s: *mut c_void,
                                                  game_handle: *mut c_void);
    fn fake_looper_set_jni_support(support: *mut c_void);
    fn fake_looper_set_rust_jni_support(support: *mut c_void);
    fn fake_assetmanager_create_and_set_global(root_dir: *const i8);

    // Varargs liblog entry points kept in C++ (`android_log_varargs.cpp`); their
    // addresses are registered as the `liblog.so` stub symbols below.
    fn __android_log_print();
    fn __android_log_vprint();
    fn __android_log_assert();
}

/// Stub GLESv2 function (was the C++ lambda `+[](void)->int{return 0;}` in capi.cpp).
/// Every name in `libGLESv2.so` maps to this address until `mc_relocate_glesv2_symbols`
/// replaces them with real GL driver entry points.
unsafe extern "C" fn glesv2_stub() -> i32 {
    0
}

pub fn setup_paths(game_dir: Option<&str>, data_dir: Option<&str>, cache_dir: Option<&str>) {
    if let Some(g) = game_dir {
        crate::path_helper::set_game_dir(g);
    }
    if let Some(d) = data_dir {
        crate::path_helper::set_data_dir(d);
    }
    if let Some(c) = cache_dir {
        crate::path_helper::set_cache_dir(c);
    }
}

pub fn init_version(package: &str, version_code: i32) {
    if let Ok(p) = CString::new(package) {
        unsafe { corelib::minecraft_version::mc_init_version(p.as_ptr(), version_code); }
    }
}

pub fn get_libc_symbols_from_cpp() -> HashMap<String, *mut c_void> {
    // Phase 10: direct call to the corelib twin (formerly mc_get_libc_symbols →
    // core_minecraft_utils_get_libc_symbols). Returns only non-null entries.
    unsafe { corelib::minecraft_utils::get_libc_symbols() }
}

pub fn setup_android_hooks() {
    unsafe { mc_setup_android_hooks() }
}

/// Phase 10 Rust port of `capi.cpp mc_load_core_libraries` (the init sequence the
/// original C++ main.cpp ran). Calls the Rust linker directly — no C++ bridge.
pub fn load_core_libraries(_lib_dir: &str) -> Result<(), i32> {
    unsafe {
        // 0) Initialize Rust linker (single owner of all libraries).
        linker::init();

        // 1) Register libc symbols with the Rust linker (corelib twin of the
        //    merged C++ + Rust getLibCSymbols + rust_load_stub("libc.so", ...)).
        // NOTE: ThreadMover::hookLibC is intentionally NOT called here.
        // The original C++ launcher runs startGame on a detached helper thread so
        // the main thread is free for executeMainThread. In the Rust bridge, both
        // startGame and executeMainThread run on the main thread. If we intercept
        // pthread_create, GameActivity_onCreate blocks waiting for the game thread
        // to signal readiness, but the thread never starts (stored in promise) → deadlock.
        // Without the hook, the game creates a real thread, GameActivity_onCreate
        // waits for it to signal readiness (which it does after ALooper_prepare),
        // then returns. The main thread blocks on executeMainThread but the game
        // thread runs the event loop and renders.
        corelib::minecraft_utils::core_minecraft_utils_register_libc_stub();

        // 2) Load libm
        corelib::minecraft_utils::core_minecraft_utils_load_lib_m();

        // 3) Setup hybris (loads libz, hooks android log, sets up mod API)
        corelib::minecraft_utils::core_minecraft_utils_setup_hybris();

        // 4) Register stub libraries that libminecraftpe.so depends on
        //
        // libHttpClient.Android.so is NOT stubbed: the real ELF from the game dir
        // is loaded by the Rust linker (matching mcpelauncher-manifest). Its
        // HCHttpCall* API routes through the JNI com.xbox.httpclient classes,
        // whose natives the Rust/C++ http_client implementation provides. The
        // linker dlsym global-scope fallback makes the Java_com_xbox_httpclient_*
        // callback symbols from libHttpClient.Android.so resolvable.
        linker::register_stub("libOpenSLES.so", &HashMap::new());
        linker::register_stub("libGLESv1_CM.so", &HashMap::new());
        linker::register_stub("libstdc++.so", &HashMap::new());

        // Register libGLESv2.so with stub functions (real GL context needed for proper symbols).
        let gl_syms: HashMap<String, *mut c_void> = GLESV2_SYMBOLS
            .iter()
            .map(|s| (s.to_string(), glesv2_stub as usize as *mut c_void))
            .collect();
        linker::register_stub("libGLESv2.so", &gl_syms);

        // EGL symbols are registered by FakeEGL::installLibrary() later, after window
        // creation. BIND_NOW requires all symbols to be present before dlopen, so
        // FakeEGL::installLibrary() must be called BEFORE mc_load_minecraft.
        // NOTE: "libEGL.so" is deliberately NOT registered here — FakeEGL handles it.
        // NOTE: android hooks (libandroid.so) and game window library are set up in
        // mc_setup_android_hooks() — call it from Rust AFTER mc_load_core_libraries
        // but BEFORE mc_load_minecraft.
        let log_syms: HashMap<String, *mut c_void> = [
            ("__android_log_print", __android_log_print as usize as *mut c_void),
            ("__android_log_vprint", __android_log_vprint as usize as *mut c_void),
            ("__android_log_write", crate::android_log_hook::__android_log_write as usize as *mut c_void),
            ("__android_log_assert", __android_log_assert as usize as *mut c_void),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_string(), v))
        .collect();
        linker::register_stub("liblog.so", &log_syms);

        // libmcpelauncher_gamewindow.so: Rust-only stub registration.
        // Full C++ registration (with callbacks) happens in
        // CorePatches::loadGameWindowLibrary().
        linker::register_stub("libmcpelauncher_gamewindow.so", &HashMap::new());
    }

    // 5) Set up library search path so dlopen_ext can find libminecraftpe.so etc.
    //    This must match the original main.cpp: update_LD_LIBRARY_PATH with the lib dir.
    let lib_dir = format!(
        "{}lib/{}",
        crate::path_helper::get_game_dir(),
        crate::path_helper::get_abi_dir()
    );
    linker::add_search_path(&lib_dir);

    Ok(())
}

pub fn create_window_and_setup_graphics() {
    unsafe { mc_create_window_and_setup_graphics() }
}

pub fn load_minecraft() -> Result<*mut c_void, ()> {
    // Phase 4: pure-Rust orchestration (port of MinecraftUtils::loadMinecraftLib).
    let handle = unsafe { crate::minecraft_load::load_minecraft() };
    if handle.is_null() { Err(()) } else { Ok(handle) }
}

pub fn create_cpp_jni_support() -> *mut c_void {
    unsafe { crate::jni_support::jni_support_create_cpp() }
}

pub fn register_minecraft_natives_cpp(support: *mut c_void,
                                      game_handle: *mut c_void) {
    unsafe { jni_support_register_minecraft_natives_cpp(support, game_handle) }
}

pub fn set_fake_looper_jni_support(support: *mut c_void) {
    unsafe { fake_looper_set_jni_support(support) }
}

pub fn set_fake_looper_rust_jni_support(support: *mut c_void) {
    unsafe { fake_looper_set_rust_jni_support(support) }
}

pub fn create_and_set_global_asset_manager(root_dir: &str) {
    let dir = CString::new(root_dir).unwrap();
    unsafe { fake_assetmanager_create_and_set_global(dir.as_ptr()) }
}

pub fn dlsym(handle: *mut c_void, symbol: &str) -> *mut c_void {
    // If handle looks like a Rust linker handle (small integer cast to ptr),
    // use Rust linker dlsym. Rust handles are < 10000.
    let handle_val = handle as usize;
    if handle_val > 0 && handle_val < 10000 {
        if let Some(addr) = linker::dlsym(handle_val, symbol) {
            return addr;
        }
    }
    let sym = CString::new(symbol).unwrap();
    unsafe { mc_dlsym(handle, sym.as_ptr()) }
}

/// Phase 10 Rust port of `capi.cpp mc_relocate_glesv2_symbols`. Called from C++
/// (`jni_bridge_stub.cpp mc_create_window_and_setup_graphics`) with
/// `fake_egl::eglGetProcAddress`; ABI matches the C `void* (*)(const char*)` type.
#[no_mangle]
pub extern "C" fn mc_relocate_glesv2_symbols(
    resolver: Option<unsafe extern "C" fn(*const c_char) -> *mut c_void>,
) {
    let mut syms: HashMap<String, *mut c_void> = HashMap::new();
    if let Some(resolve) = resolver {
        for name in GLESV2_SYMBOLS {
            let cname = CString::new(*name).unwrap();
            let addr = unsafe { resolve(cname.as_ptr()) };
            if !addr.is_null() {
                syms.insert(name.to_string(), addr);
            }
        }
    }
    if syms.is_empty() {
        eprintln!("LAUNCHER: no GLESv2 symbols resolved (missing GL driver?)");
        return;
    }
    if let Some(handle) = linker::find_library("libGLESv2.so") {
        linker::add_symbols(handle, &syms);
    }
    eprintln!(
        "LAUNCHER: relocated {} GLESv2 symbols into Rust linker",
        syms.len()
    );
}

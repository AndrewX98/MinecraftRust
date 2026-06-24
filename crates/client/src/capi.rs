//! C bridge to compiled C++ mcpelauncher code.
//! Functions are defined in capi.cpp and linked from the cmake-built .a files.

use std::ffi::{CStr, CString};

extern "C" {
    fn mc_setup_paths(game_dir: *const i8, data_dir: *const i8, cache_dir: *const i8);
    fn mc_init_version(package: *const i8, version_code: i32);
    fn mc_get_libc_symbols(buf: *mut libc_shim::types::shimmed_symbol, max_entries: i32) -> i32;
    fn mc_load_core_libraries(lib_dir: *const i8) -> i32;
    fn mc_load_minecraft() -> *mut std::ffi::c_void;
    fn mc_setup_android_hooks();
    fn mc_create_window_and_setup_graphics();
    pub fn mc_egl_swap_buffers(display: *mut std::ffi::c_void, surface: *mut std::ffi::c_void) -> i32;
    fn mc_dlsym(handle: *mut std::ffi::c_void, symbol: *const i8) -> *mut std::ffi::c_void;
    fn jni_support_start_game_cpp(s: *mut std::ffi::c_void, game_on_create: *mut std::ffi::c_void,
                                  stbi_load: *mut std::ffi::c_void, stbi_image_free: *mut std::ffi::c_void);
    fn jni_support_register_minecraft_natives_cpp(s: *mut std::ffi::c_void,
                                                  game_handle: *mut std::ffi::c_void);
    fn fake_looper_set_jni_support(support: *mut std::ffi::c_void);
    fn fake_looper_set_rust_jni_support(support: *mut std::ffi::c_void);
    fn fake_assetmanager_create_and_set_global(root_dir: *const i8);
}

pub fn setup_paths(game_dir: Option<&str>, data_dir: Option<&str>, cache_dir: Option<&str>) {
    let g = game_dir.and_then(|s| CString::new(s).ok());
    let d = data_dir.and_then(|s| CString::new(s).ok());
    let c = cache_dir.and_then(|s| CString::new(s).ok());
    unsafe {
        mc_setup_paths(
            g.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            d.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
            c.as_ref().map_or(std::ptr::null(), |s| s.as_ptr()),
        );
    }
}

pub fn init_version(package: &str, version_code: i32) {
    if let Ok(p) = CString::new(package) {
        unsafe { mc_init_version(p.as_ptr(), version_code); }
    }
}

pub fn get_libc_symbols_from_cpp() -> std::collections::HashMap<String, *mut std::ffi::c_void> {
    let mut syms = std::collections::HashMap::new();
    let max = 700i32;
    let mut buf: Vec<libc_shim::types::shimmed_symbol> = Vec::with_capacity(max as usize);
    unsafe {
        buf.set_len(max as usize);
        let count = mc_get_libc_symbols(buf.as_mut_ptr(), max);
        for sym in buf.iter().take(count as usize) {
            if !sym.name.is_null() && !sym.value.is_null() {
                let name = CStr::from_ptr(sym.name).to_str().unwrap_or("").to_string();
                syms.insert(name, sym.value);
            }
        }
    }
    syms
}

pub fn setup_android_hooks() {
    unsafe { mc_setup_android_hooks() }
}

pub fn load_core_libraries(lib_dir: &str) -> Result<(), i32> {
    let dir = CString::new(lib_dir).unwrap();
    let rc = unsafe { mc_load_core_libraries(dir.as_ptr()) };
    if rc == 0 { Ok(()) } else { Err(rc) }
}

pub fn create_window_and_setup_graphics() {
    unsafe { mc_create_window_and_setup_graphics() }
}

pub fn load_minecraft() -> Result<*mut std::ffi::c_void, ()> {
    let handle = unsafe { mc_load_minecraft() };
    if handle.is_null() { Err(()) } else { Ok(handle) }
}

pub fn create_cpp_jni_support() -> *mut std::ffi::c_void {
    unsafe { crate::jni_support::jni_support_create_cpp() }
}

pub fn destroy_cpp_jni_support(s: *mut std::ffi::c_void) {
    unsafe { crate::jni_support::jni_support_destroy_cpp(s) }
}

pub fn start_game_cpp(support: *mut std::ffi::c_void, game_on_create: *mut std::ffi::c_void,
                      stbi_load: *mut std::ffi::c_void, stbi_image_free: *mut std::ffi::c_void) {
    unsafe { jni_support_start_game_cpp(support, game_on_create, stbi_load, stbi_image_free) }
}

pub fn register_minecraft_natives_cpp(support: *mut std::ffi::c_void,
                                      game_handle: *mut std::ffi::c_void) {
    unsafe { jni_support_register_minecraft_natives_cpp(support, game_handle) }
}

pub fn set_fake_looper_jni_support(support: *mut std::ffi::c_void) {
    unsafe { fake_looper_set_jni_support(support) }
}

pub fn set_fake_looper_rust_jni_support(support: *mut std::ffi::c_void) {
    unsafe { fake_looper_set_rust_jni_support(support) }
}

pub fn create_and_set_global_asset_manager(root_dir: &str) {
    let dir = CString::new(root_dir).unwrap();
    unsafe { fake_assetmanager_create_and_set_global(dir.as_ptr()) }
}

pub fn dlsym(handle: *mut std::ffi::c_void, symbol: &str) -> *mut std::ffi::c_void {
    let sym = CString::new(symbol).unwrap();
    unsafe { mc_dlsym(handle, sym.as_ptr()) }
}

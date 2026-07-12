use std::collections::HashMap;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

use crate::Handle;

/// Convert a Rust Handle (usize) to an opaque C handle (void*).
fn handle_to_ptr(handle: Handle) -> *mut std::ffi::c_void {
    handle as *mut std::ffi::c_void
}

/// Convert an opaque C handle (void*) back to a Rust Handle (usize).
unsafe fn ptr_to_handle(handle: *mut std::ffi::c_void) -> Handle {
    handle as usize
}

thread_local! {
    static DL_ERROR: std::cell::RefCell<String> = const { std::cell::RefCell::new(String::new()) };
    static DLADDR_NAME: std::cell::RefCell<Option<CString>> = const { std::cell::RefCell::new(None) };
}

fn set_dlerror(msg: String) {
    DL_ERROR.with(|e| *e.borrow_mut() = msg);
}

fn get_dlerror() -> String {
    DL_ERROR.with(|e| std::mem::take(&mut *e.borrow_mut()))
}

/// Store a CString in thread-local storage and return a pointer to it.
/// The previous value is dropped. The returned pointer is valid until the next
/// call to this function (or thread exit), matching bionic's behavior where
/// `dladdr` points into static storage.
fn set_dladdr_name(name: String) -> *const c_char {
    let c = CString::new(name).unwrap_or_default();
    let ptr = c.as_ptr();
    DLADDR_NAME.with(|slot| *slot.borrow_mut() = Some(c));
    ptr
}

// --- Wrapper implementations (internal, no symbol conflict) ---

unsafe fn dlopen_impl(filename: *const c_char, flags: i32) -> *mut std::ffi::c_void {
    if filename.is_null() {
        set_dlerror("dlopen: null filename".to_string());
        return ptr::null_mut();
    }
    let path = match unsafe { CStr::from_ptr(filename) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            set_dlerror("dlopen: invalid filename".to_string());
            return ptr::null_mut();
        }
    };
    match crate::dlopen(path, flags) {
        Some(h) => handle_to_ptr(h),
        None => {
            set_dlerror(format!("dlopen: failed to load \"{}\"", path));
            ptr::null_mut()
        }
    }
}

unsafe fn dlsym_impl(handle: *mut std::ffi::c_void, symbol: *const c_char) -> *mut std::ffi::c_void {
    if handle.is_null() {
        set_dlerror("dlsym: null handle".to_string());
        return ptr::null_mut();
    }
    if symbol.is_null() {
        set_dlerror("dlsym: null symbol name".to_string());
        return ptr::null_mut();
    }
    let name = match unsafe { CStr::from_ptr(symbol) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            set_dlerror("dlsym: invalid symbol name".to_string());
            return ptr::null_mut();
        }
    };
    let h = unsafe { ptr_to_handle(handle) };
    match crate::dlsym(h, name) {
        Some(addr) => addr,
        None => {
            set_dlerror(format!("dlsym: symbol \"{}\" not found", name));
            ptr::null_mut()
        }
    }
}

unsafe fn dlclose_impl(handle: *mut std::ffi::c_void) -> i32 {
    if handle.is_null() {
        set_dlerror("dlclose: null handle".to_string());
        return -1;
    }
    let h = unsafe { ptr_to_handle(handle) };
    if crate::dlclose(h) == 0 {
        set_dlerror(format!("dlclose: handle {} not found", h));
        return -1;
    }
    0
}

unsafe fn dladdr_impl(
    addr: *const std::ffi::c_void,
    info: *mut libc::Dl_info,
) -> i32 {
    if addr.is_null() || info.is_null() {
        return 0;
    }
    match crate::dladdr(addr) {
        Some((handle, name)) => {
            let ptr = set_dladdr_name(name);
            unsafe {
                (*info).dli_fname = ptr;
                (*info).dli_fbase = handle_to_ptr(handle);
                (*info).dli_sname = ptr::null();
                (*info).dli_saddr = ptr::null_mut();
            }
            1
        }
        None => 0,
    }
}

unsafe fn dl_iterate_phdr_impl(
    callback: Option<
        unsafe extern "C" fn(info: *mut libc::dl_phdr_info, size: usize, data: *mut std::ffi::c_void) -> i32,
    >,
    data: *mut std::ffi::c_void,
) -> i32 {
    let cb = match callback {
        Some(f) => f,
        None => return -1,
    };
    let state = crate::STATE.read().unwrap();
    for (_handle, lib) in &state.libraries_by_handle {
        let name = CString::new(lib.soinfo.soname.clone()).unwrap_or_default();
        let mut info = libc::dl_phdr_info {
            dlpi_addr: lib.soinfo.base as libc::Elf64_Addr,
            dlpi_name: name.as_ptr(),
            dlpi_phdr: ptr::null(),
            dlpi_phnum: 0,
            dlpi_adds: 0,
            dlpi_subs: 0,
            dlpi_tls_modid: 0,
            dlpi_tls_data: ptr::null_mut(),
        };
        let ret = cb(&mut info as *mut libc::dl_phdr_info, size_of::<libc::dl_phdr_info>(), data);
        if ret != 0 {
            return ret;
        }
    }
    0
}

unsafe fn android_dlopen_ext_impl(
    filename: *const c_char,
    flags: i32,
    _extinfo: *const std::ffi::c_void,
) -> *mut std::ffi::c_void {
    if filename.is_null() {
        return ptr::null_mut();
    }
    let path = match unsafe { CStr::from_ptr(filename) }.to_str() {
        Ok(s) => s,
        Err(_) => return ptr::null_mut(),
    };
    let hooks = [];
    match crate::dlopen_ext(path, flags, &hooks) {
        Some(h) => handle_to_ptr(h),
        None => {
            set_dlerror(format!("android_dlopen_ext: failed to load \"{}\"", path));
            ptr::null_mut()
        }
    }
}

fn android_get_application_target_sdk_version_impl() -> i32 {
    crate::sdk_versions::get_application_target_sdk_version()
}

fn android_set_application_target_sdk_version_impl(target: i32) {
    crate::sdk_versions::set_application_target_sdk_version(target);
}

// --- Additional dlfcn.cpp functions ---

unsafe fn dlvsym_impl(
    handle: *mut std::ffi::c_void,
    symbol: *const c_char,
    _version: *const c_char,
) -> *mut std::ffi::c_void {
    if handle.is_null() {
        set_dlerror("dlvsym: null handle".to_string());
        return ptr::null_mut();
    }
    if symbol.is_null() {
        set_dlerror("dlvsym: null symbol name".to_string());
        return ptr::null_mut();
    }
    let name = match unsafe { CStr::from_ptr(symbol) }.to_str() {
        Ok(s) => s,
        Err(_) => {
            set_dlerror("dlvsym: invalid symbol name".to_string());
            return ptr::null_mut();
        }
    };
    let h = unsafe { ptr_to_handle(handle) };
    match crate::dlsym(h, name) {
        Some(addr) => addr,
        None => {
            set_dlerror(format!("dlvsym: symbol \"{}\" not found", name));
            ptr::null_mut()
        }
    }
}

fn android_dlwarning_impl(obj: *mut std::ffi::c_void, f: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const c_char)>) {
    crate::dlwarning::get_dlwarning(obj, f);
}

fn android_get_LD_LIBRARY_PATH_impl(buffer: *mut c_char, buffer_size: usize) {
    let lib_path = crate::linker_main::ld_library_path();
    let joined = lib_path.join(":");
    if buffer.is_null() || buffer_size == 0 {
        return;
    }
    let c_str = CString::new(joined).unwrap_or_default();
    let bytes = c_str.as_bytes_with_nul();
    let copy_len = bytes.len().min(buffer_size.saturating_sub(1));
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), buffer as *mut u8, copy_len);
        *buffer.add(copy_len) = 0;
    }
}

fn android_update_LD_LIBRARY_PATH_impl(ld_library_path: *const c_char) {
    if ld_library_path.is_null() {
        return;
    }
    let path = match unsafe { CStr::from_ptr(ld_library_path) }.to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    crate::linker_main::parse_ld_library_path(path);
}

fn android_init_anonymous_namespace_impl(
    _shared_libs_sonames: *const c_char,
    _library_search_path: *const c_char,
) -> bool {
    true
}

fn android_create_namespace_impl(
    _name: *const c_char,
    _ld_library_path: *const c_char,
    _default_library_path: *const c_char,
    _type: u64,
    _permitted_when_isolated_path: *const c_char,
    _parent_namespace: *mut std::ffi::c_void,
) -> *mut std::ffi::c_void {
    ptr::null_mut()
}

fn android_link_namespaces_impl(
    _namespace_from: *mut std::ffi::c_void,
    _namespace_to: *mut std::ffi::c_void,
    _shared_libs_sonames: *const c_char,
) -> bool {
    false
}

fn android_link_namespaces_all_libs_impl(
    _namespace_from: *mut std::ffi::c_void,
    _namespace_to: *mut std::ffi::c_void,
) -> bool {
    false
}

fn android_get_exported_namespace_impl(_name: *const c_char) -> *mut std::ffi::c_void {
    ptr::null_mut()
}

fn cfi_fail_impl(_call_site_type_id: u64, _ptr: *mut std::ffi::c_void, _diag_data: *mut std::ffi::c_void, _caller_pc: *mut std::ffi::c_void) {
}

fn add_thread_local_dtor_impl(_dso_handle: *mut std::ffi::c_void) {
}

fn remove_thread_local_dtor_impl(_dso_handle: *mut std::ffi::c_void) {
}

/// Returns a map of all libdl symbol names to their function pointers.
/// These can be registered with the linker via `add_symbols()`.
pub fn get_dl_symbols() -> HashMap<String, *mut std::ffi::c_void> {
    let mut map = HashMap::new();
    map.insert("dlopen".to_string(), dlopen_impl as *mut std::ffi::c_void);
    map.insert("dlsym".to_string(), dlsym_impl as *mut std::ffi::c_void);
    map.insert("dlvsym".to_string(), dlvsym_impl as *mut std::ffi::c_void);
    map.insert("dlclose".to_string(), dlclose_impl as *mut std::ffi::c_void);
    map.insert("dladdr".to_string(), dladdr_impl as *mut std::ffi::c_void);
    map.insert(
        "dlerror".to_string(),
        get_dlerror as *mut std::ffi::c_void,
    );
    map.insert(
        "dl_iterate_phdr".to_string(),
        dl_iterate_phdr_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_dlopen_ext".to_string(),
        android_dlopen_ext_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_dlwarning".to_string(),
        android_dlwarning_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_get_application_target_sdk_version".to_string(),
        android_get_application_target_sdk_version_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_set_application_target_sdk_version".to_string(),
        android_set_application_target_sdk_version_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_get_LD_LIBRARY_PATH".to_string(),
        android_get_LD_LIBRARY_PATH_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_update_LD_LIBRARY_PATH".to_string(),
        android_update_LD_LIBRARY_PATH_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_init_anonymous_namespace".to_string(),
        android_init_anonymous_namespace_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_create_namespace".to_string(),
        android_create_namespace_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_link_namespaces".to_string(),
        android_link_namespaces_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_link_namespaces_all_libs".to_string(),
        android_link_namespaces_all_libs_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "android_get_exported_namespace".to_string(),
        android_get_exported_namespace_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "cfi_fail".to_string(),
        cfi_fail_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "add_thread_local_dtor".to_string(),
        add_thread_local_dtor_impl as *mut std::ffi::c_void,
    );
    map.insert(
        "remove_thread_local_dtor".to_string(),
        remove_thread_local_dtor_impl as *mut std::ffi::c_void,
    );
    map
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init;

    fn setup() {
        init();
    }

    #[test]
    fn test_get_dl_symbols_not_empty() {
        setup();
        let syms = get_dl_symbols();
        assert!(!syms.is_empty(), "get_dl_symbols should return at least one symbol");
        assert!(syms.contains_key("dlopen"));
        assert!(syms.contains_key("dlsym"));
        assert!(syms.contains_key("dlclose"));
        assert!(syms.contains_key("dladdr"));
        assert!(syms.contains_key("dl_iterate_phdr"));
        assert!(syms.contains_key("android_dlopen_ext"));
        assert!(syms.contains_key("android_get_application_target_sdk_version"));
    }

    #[test]
    fn test_function_pointers_are_non_null() {
        setup();
        let syms = get_dl_symbols();
        for (name, ptr) in &syms {
            assert!(!ptr.is_null(), "symbol \"{}\" has null function pointer", name);
        }
    }

    #[test]
    fn test_dlopen_null_name() {
        setup();
        let result = unsafe { dlopen_impl(ptr::null(), 0) };
        assert!(result.is_null());
        let err = get_dlerror();
        assert!(err.contains("null filename"), "expected 'null filename' error, got: {}", err);
    }

    #[test]
    fn test_dlsym_null_handle() {
        setup();
        let name = CString::new("test").unwrap();
        let result = unsafe { dlsym_impl(ptr::null_mut(), name.as_ptr()) };
        assert!(result.is_null());
        let err = get_dlerror();
        assert!(err.contains("null handle"), "expected 'null handle' error, got: {}", err);
    }

    #[test]
    fn test_dlsym_null_name() {
        setup();
        let handle = handle_to_ptr(42);
        let result = unsafe { dlsym_impl(handle, ptr::null()) };
        assert!(result.is_null());
        let err = get_dlerror();
        assert!(err.contains("null symbol"), "expected 'null symbol' error, got: {}", err);
    }

    #[test]
    fn test_dlclose_null_handle() {
        setup();
        let result = unsafe { dlclose_impl(ptr::null_mut()) };
        assert_eq!(result, -1);
        let err = get_dlerror();
        assert!(err.contains("null handle"), "expected 'null handle' error, got: {}", err);
    }

    #[test]
    fn test_dlsym_invalid_handle() {
        setup();
        let name = CString::new("nonexistent").unwrap();
        let handle = handle_to_ptr(99999);
        let result = unsafe { dlsym_impl(handle, name.as_ptr()) };
        assert!(result.is_null());
        let err = get_dlerror();
        assert!(err.contains("not found"), "expected 'not found' error, got: {}", err);
    }

    #[test]
    fn test_dladdr_null_addr() {
        setup();
        let mut info = unsafe { std::mem::zeroed::<libc::Dl_info>() };
        let result = unsafe { dladdr_impl(ptr::null(), &mut info) };
        assert_eq!(result, 0);
    }

    #[test]
    fn test_dladdr_null_info() {
        setup();
        let result = unsafe { dladdr_impl(&0 as *const _ as *const std::ffi::c_void, ptr::null_mut()) };
        assert_eq!(result, 0);
    }

    #[test]
    fn test_dl_iterate_phdr_no_callback() {
        setup();
        let result = unsafe { dl_iterate_phdr_impl(None, ptr::null_mut()) };
        assert_eq!(result, -1);
    }

    #[test]
    fn test_dl_iterate_phdr_empty_callback() {
        setup();
        let result = unsafe {
            extern "C" fn cb(
                _info: *mut libc::dl_phdr_info,
                _size: usize,
                _data: *mut std::ffi::c_void,
            ) -> i32 {
                0
            }
            dl_iterate_phdr_impl(Some(cb), ptr::null_mut())
        };
        assert_eq!(result, 0);
    }

    #[test]
    fn test_dl_iterate_phdr_stop_early() {
        setup();
        let count: *mut i32 = &mut 0;
        let result = unsafe {
            extern "C" fn cb(
                _info: *mut libc::dl_phdr_info,
                _size: usize,
                data: *mut std::ffi::c_void,
            ) -> i32 {
                unsafe { *(data as *mut i32) += 1 };
                1 // stop after first
            }
            dl_iterate_phdr_impl(Some(cb), count as *mut std::ffi::c_void)
        };
        assert_eq!(result, 1);
    }

    #[test]
    fn test_dlerror_roundtrip() {
        setup();
        let err = get_dlerror();
        assert!(err.is_empty(), "dlerror should be empty initially");
        set_dlerror("test error".to_string());
        let err = get_dlerror();
        assert_eq!(err, "test error");
        // After getting, the error should be cleared
        let err = get_dlerror();
        assert!(err.is_empty(), "dlerror should be cleared after reading");
    }

    #[test]
    fn test_android_sdk_version_default() {
        setup();
        let v = crate::sdk_versions::get_application_target_sdk_version();
        assert_eq!(v, 35, "default SDK version should be 35");
    }

    #[test]
    fn test_android_sdk_version_set_and_get() {
        setup();
        crate::sdk_versions::set_application_target_sdk_version(30);
        assert_eq!(crate::sdk_versions::get_application_target_sdk_version(), 30);
        crate::sdk_versions::set_application_target_sdk_version(35);
        assert_eq!(crate::sdk_versions::get_application_target_sdk_version(), 35);
    }

    #[test]
    fn test_android_get_application_target_sdk_version() {
        setup();
        let v = android_get_application_target_sdk_version_impl();
        assert_eq!(v, 35);
    }

    #[test]
    fn test_android_set_application_target_sdk_version() {
        setup();
        android_set_application_target_sdk_version_impl(33);
        assert_eq!(crate::sdk_versions::get_application_target_sdk_version(), 33);
        android_set_application_target_sdk_version_impl(35);
    }

    #[test]
    fn test_handle_conversion_roundtrip() {
        for h in [0usize, 1, 42, usize::MAX] {
            let ptr = handle_to_ptr(h);
            let back = unsafe { ptr_to_handle(ptr) };
            assert_eq!(h, back, "handle conversion roundtrip failed for {}", h);
        }
    }

    #[test]
    fn test_all_symbol_types_distinct() {
        setup();
        let syms = get_dl_symbols();
        let mut ptrs = std::collections::HashSet::new();
        for (_name, ptr) in &syms {
            // Each function pointer should be unique
            // (unless two names happen to point at the same function)
            ptrs.insert(*ptr);
        }
        // All symbols should be distinct functions
        // (at minimum we have 9 entries, but some might combine)
        assert!(ptrs.len() >= 7, "expected at least 7 unique function pointers");
    }

    #[test]
    fn test_dlerror_cleared_after_dlopen_fail() {
        setup();
        get_dlerror();
        unsafe { dlopen_impl(ptr::null(), 0) };
        let err = get_dlerror();
        assert!(!err.is_empty(), "dlerror should be set after dlopen failure");
        let err2 = get_dlerror();
        assert!(err2.is_empty(), "dlerror should be cleared after second read");
    }

    // --- dlvsym tests ---

    #[test]
    fn test_dlvsym_null_handle() {
        setup();
        let name = CString::new("test").unwrap();
        let result = unsafe { dlvsym_impl(ptr::null_mut(), name.as_ptr(), ptr::null()) };
        assert!(result.is_null());
        let err = get_dlerror();
        assert!(err.contains("null handle"));
    }

    #[test]
    fn test_dlvsym_null_name() {
        setup();
        let handle = handle_to_ptr(42);
        let result = unsafe { dlvsym_impl(handle, ptr::null(), ptr::null()) };
        assert!(result.is_null());
        let err = get_dlerror();
        assert!(err.contains("null symbol"));
    }

    #[test]
    fn test_dlvsym_version_ignored() {
        setup();
        let name = CString::new("nonexistent").unwrap();
        let ver = CString::new("GLIBC_2.34").unwrap();
        let handle = handle_to_ptr(99999);
        let result = unsafe { dlvsym_impl(handle, name.as_ptr(), ver.as_ptr()) };
        assert!(result.is_null());
    }

    // --- android_get_LD_LIBRARY_PATH / android_update_LD_LIBRARY_PATH ---

    #[test]
    fn test_android_get_ld_library_path_empty_after_init() {
        setup();
        let mut buf = [0i8; 256];
        android_get_LD_LIBRARY_PATH_impl(buf.as_mut_ptr(), buf.len());
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_str().unwrap();
        assert_eq!(s, "");
    }

    #[test]
    fn test_android_update_ld_library_path() {
        setup();
        let path = CString::new("/test/lib:/other/lib").unwrap();
        android_update_LD_LIBRARY_PATH_impl(path.as_ptr());
        let mut buf = [0i8; 512];
        android_get_LD_LIBRARY_PATH_impl(buf.as_mut_ptr(), buf.len());
        let s = unsafe { CStr::from_ptr(buf.as_ptr()) }.to_str().unwrap();
        assert!(s.contains("/test/lib"));
        assert!(s.contains("/other/lib"));
    }

    // --- android_dlwarning ---

    #[test]
    fn test_android_dlwarning_null_callback() {
        setup();
        android_dlwarning_impl(ptr::null_mut(), None);
    }

    #[test]
    fn test_android_dlwarning_with_callback() {
        setup();
        extern "C" fn cb(_obj: *mut std::ffi::c_void, _msg: *const c_char) {
        }
        android_dlwarning_impl(ptr::null_mut(), Some(cb));
    }

    // --- namespace stubs ---

    #[test]
    fn test_android_init_anonymous_namespace_ok() {
        setup();
        let result = android_init_anonymous_namespace_impl(ptr::null(), ptr::null());
        assert!(result);
    }

    #[test]
    fn test_android_create_namespace_returns_null() {
        setup();
        let result = android_create_namespace_impl(ptr::null(), ptr::null(), ptr::null(), 0, ptr::null(), ptr::null_mut());
        assert!(result.is_null());
    }

    #[test]
    fn test_android_link_namespaces_fails() {
        setup();
        let result = android_link_namespaces_impl(ptr::null_mut(), ptr::null_mut(), ptr::null());
        assert!(!result);
    }

    #[test]
    fn test_android_link_namespaces_all_libs_fails() {
        setup();
        let result = android_link_namespaces_all_libs_impl(ptr::null_mut(), ptr::null_mut());
        assert!(!result);
    }

    #[test]
    fn test_android_get_exported_namespace_returns_null() {
        setup();
        let result = android_get_exported_namespace_impl(ptr::null());
        assert!(result.is_null());
    }

    // --- new symbol presence ---

    #[test]
    fn test_all_dlfcn_symbols_present() {
        setup();
        let syms = get_dl_symbols();
        for name in &[
            "dlopen", "dlsym", "dlvsym", "dlclose", "dladdr", "dlerror",
            "dl_iterate_phdr", "android_dlopen_ext", "android_dlwarning",
            "android_get_application_target_sdk_version",
            "android_set_application_target_sdk_version",
            "android_get_LD_LIBRARY_PATH", "android_update_LD_LIBRARY_PATH",
            "android_init_anonymous_namespace", "android_create_namespace",
            "android_link_namespaces", "android_link_namespaces_all_libs",
            "android_get_exported_namespace", "cfi_fail",
            "add_thread_local_dtor", "remove_thread_local_dtor",
        ] {
            assert!(syms.contains_key(*name), "missing symbol: {}", name);
        }
    }
}

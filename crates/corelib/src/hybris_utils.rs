use std::ffi::{c_char, c_void, CStr, CString};

extern "C" {
    fn linker_load_library_rust(
        name: *const c_char,
        keys: *const *const c_char,
        vals: *const *mut c_void,
        len: usize,
    ) -> usize;
}

/// Extra symbol overrides for `mc_hybris_load_library_os`, mirroring the
/// by-value `std::unordered_map<std::string, void*>` argument of the C++
/// `HybrisUtils::loadLibraryOS` 4-arg overload. Used on Apple to inject
/// `sincos`/`sincosf`; unused on Linux.
#[repr(C)]
pub struct HybrisSym {
    pub name: *const c_char,
    pub addr: *mut c_void,
}

/// Walk a null-terminated `const char**` symbol list into owned names.
///
/// Faithful port of the `while (symbols[i])` walk used by both C++
/// `HybrisUtils::loadLibraryOS` and `stubSymbols`. Returns `None` on a null
/// list pointer; an empty list yields an empty vector.
pub unsafe fn collect_symbol_names(symbols: *mut *const c_char) -> Option<Vec<String>> {
    if symbols.is_null() {
        return None;
    }
    let mut out = Vec::new();
    let mut i = 0usize;
    loop {
        let sym = *symbols.add(i);
        if sym.is_null() {
            break;
        }
        out.push(CStr::from_ptr(sym).to_string_lossy().into_owned());
        i += 1;
    }
    Some(out)
}

/// `#[no_mangle]` twin of `HybrisUtils::loadLibraryOS` (`hybris_utils.cpp:9`).
/// `dlopen`s `path`, resolves every name in `symbols` via `dlsym`, merges the
/// optional `extra_syms` overrides, and hands the map to the Rust linker via
/// `linker_load_library_rust`. Returns the OS handle, or NULL on failure.
#[no_mangle]
pub unsafe extern "C" fn mc_hybris_load_library_os(
    name: *const c_char,
    path: *const c_char,
    symbols: *mut *const c_char,
    extra_syms: *const HybrisSym,
    extra_syms_len: usize,
) -> *mut c_void {
    if name.is_null() || path.is_null() {
        return std::ptr::null_mut();
    }
    let name = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let path = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let path_c = match CString::new(path) {
        Ok(c) => c,
        Err(_) => return std::ptr::null_mut(),
    };
    let handle = unsafe { libc::dlopen(path_c.as_ptr(), libc::RTLD_LAZY) };
    if handle.is_null() {
        log::error!("LinkerUtils: Failed to load OS library {}", path);
        return std::ptr::null_mut();
    }
    log::trace!("LinkerUtils: Loaded OS library {}", path);

    let mut keys: Vec<CString> = Vec::new();
    let mut vals: Vec<*mut c_void> = Vec::new();
    if let Some(names) = collect_symbol_names(symbols) {
        for n in names {
            let sym_c = match CString::new(n.as_bytes()) {
                Ok(c) => c,
                Err(_) => continue,
            };
            let ptr = unsafe { libc::dlsym(handle, sym_c.as_ptr()) };
            if !ptr.is_null() {
                keys.push(sym_c);
                vals.push(ptr);
            }
        }
    }
    for i in 0..extra_syms_len {
        let e = unsafe { &*extra_syms.add(i) };
        if e.name.is_null() {
            continue;
        }
        if let Ok(n) = unsafe { CStr::from_ptr(e.name).to_str() } {
            if let Ok(c) = CString::new(n.as_bytes()) {
                keys.push(c);
                vals.push(e.addr);
            }
        }
    }
    if !keys.is_empty() {
        let key_ptrs: Vec<*const c_char> = keys.iter().map(|c| c.as_ptr()).collect();
        let val_ptrs: Vec<*mut c_void> = vals.iter().copied().collect();
        let name_c = CString::new(name).expect("no interior NUL");
        unsafe {
            linker_load_library_rust(name_c.as_ptr(), key_ptrs.as_ptr(), val_ptrs.as_ptr(), key_ptrs.len());
        }
    }
    handle
}

/// `#[no_mangle]` twin of `HybrisUtils::stubSymbols` (`hybris_utils.cpp:47`).
/// Registers a single stub function under every name in `symbols` with the Rust
/// linker.
#[no_mangle]
pub unsafe extern "C" fn mc_hybris_stub_symbols(
    name: *const c_char,
    symbols: *mut *const c_char,
    stub: *mut c_void,
) {
    if name.is_null() || stub.is_null() {
        return;
    }
    let name = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    let mut keys: Vec<CString> = Vec::new();
    let mut vals: Vec<*mut c_void> = Vec::new();
    if let Some(names) = collect_symbol_names(symbols) {
        for n in names {
            if let Ok(c) = CString::new(n.as_bytes()) {
                keys.push(c);
                vals.push(stub);
            }
        }
    }
    if keys.is_empty() {
        return;
    }
    let key_ptrs: Vec<*const c_char> = keys.iter().map(|c| c.as_ptr()).collect();
    let val_ptrs: Vec<*mut c_void> = vals.iter().copied().collect();
    let name_c = CString::new(name).expect("no interior NUL");
    unsafe {
        linker_load_library_rust(name_c.as_ptr(), key_ptrs.as_ptr(), val_ptrs.as_ptr(), key_ptrs.len());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collect_symbols_in_order() {
        let names = [c"malloc", c"free", c"sincos"];
        let mut ptrs: Vec<*const c_char> = names.iter().map(|c| c.as_ptr()).collect();
        ptrs.push(std::ptr::null());
        let got = unsafe { collect_symbol_names(ptrs.as_mut_ptr()).unwrap() };
        assert_eq!(got, vec!["malloc", "free", "sincos"]);
    }

    #[test]
    fn collect_symbols_empty_list() {
        let mut ptrs: Vec<*const c_char> = vec![std::ptr::null()];
        let got = unsafe { collect_symbol_names(ptrs.as_mut_ptr()).unwrap() };
        assert_eq!(got, Vec::<String>::new());
    }

    #[test]
    fn collect_symbols_null_pointer() {
        let got = unsafe { collect_symbol_names(std::ptr::null_mut()) };
        assert!(got.is_none());
    }
}
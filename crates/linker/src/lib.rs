pub mod base_strings;
pub mod block_allocator;
pub mod cfi;
pub mod debug;
pub mod dlwarning;
pub mod gdb_support;
pub mod linker_config;
pub mod linker_main;
pub mod linker_stubs;
pub mod loader;
pub mod phdr;
pub mod mapped_file_fragment;
pub mod reloc;
pub mod relocate;
pub mod reloc_iter;
pub mod sdk_versions;
pub mod tls;
pub mod soinfo;
pub mod symbol;
pub mod libdl;
pub mod namespaces;
pub mod properties;
pub mod utils;

use soinfo::SoInfo;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

pub type Handle = usize;

#[derive(Clone)]
pub struct LoadedLibrary {
    pub soinfo: SoInfo,
    pub ref_count: u32,
    pub is_stub: bool,
    pub is_linked: bool,
}

struct LinkerState {
    libraries_by_handle: HashMap<Handle, LoadedLibrary>,
    libraries_by_name: HashMap<String, Handle>,
    global_symbols: HashMap<String, usize>,
    next_handle: Handle,
    search_paths: Vec<String>,
}

impl LinkerState {
    fn new() -> Self {
        Self {
            libraries_by_handle: HashMap::new(),
            libraries_by_name: HashMap::new(),
            global_symbols: HashMap::new(),
            next_handle: 1,
            search_paths: Vec::new(),
        }
    }
}

/// Optional C++ dlsym fallback for symbols not found in the Rust linker state.
/// Set by `linker_rust_set_dlsym_fallback` from C++.
static DLSYM_FALLBACK: std::sync::OnceLock<unsafe extern "C" fn(*const libc::c_char) -> *mut libc::c_void> =
    std::sync::OnceLock::new();

#[derive(Debug, thiserror::Error)]
pub enum LinkerError {
    #[error("Library not found: {0}")]
    LibraryNotFound(String),
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
    #[error("Load error: {0}")]
    Load(String),
}

static STATE: LazyLock<RwLock<LinkerState>> = LazyLock::new(|| RwLock::new(LinkerState::new()));

pub fn resolve_symbol(name: &str) -> Option<usize> {
    let state = STATE.read().unwrap();
    if let Some(&addr) = state.global_symbols.get(name) {
        return Some(addr);
    }
    for (_, lib) in &state.libraries_by_handle {
        if lib.is_stub {
            continue;
        }
        if let Some((addr, _)) = symbol::find_symbol(&lib.soinfo, name) {
            if addr != 0 {
                return Some(addr);
            }
        }
        if let Some(&addr) = lib.soinfo.external_symbols.get(name) {
            return Some(addr);
        }
    }
    None
}

pub fn init() {
    static ONCE: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    ONCE.get_or_init(|| {
        log::info!("linker: initializing");
        *STATE.write().unwrap() = LinkerState::new();
        let dl_syms = libdl::get_dl_symbols();
        load_library_internal("libdl.so", &dl_syms, true);
        log::info!("linker: initialized");
    });
}

fn load_dependencies(
    soinfo: &mut SoInfo,
    _data: &[u8],
    name: &str,
    external_symbols: &HashMap<String, *mut std::ffi::c_void>,
) {
    let deps = soinfo.dependencies.clone();
    log::info!("linker: {} has {} deps: {:?}", name, deps.len(), deps);
    for dep_name in &deps {
        if dep_name == "libc.so" {
            continue;
        }
        if dep_name == "libm.so" || dep_name == "libdl.so" || dep_name == "libz.so" {
            continue;
        }
        if dep_name == "libGLESv2.so" || dep_name == "libOpenSLES.so" || dep_name == "libstdc++.so" {
            continue;
        }
        if is_loaded(dep_name) {
            continue;
        }
        log::debug!("linker: loading dependency {} for {}", dep_name, name);
        load_library_internal(dep_name, external_symbols, false);
    }
}

fn is_loaded(name: &str) -> bool {
    let state = STATE.read().unwrap();
    state.libraries_by_name.contains_key(name)
}

pub fn load_library(name: &str, symbols: &HashMap<String, *mut std::ffi::c_void>) -> Handle {
    load_library_internal(name, symbols, false)
}

fn load_library_internal(
    name_: &str,
    external_symbols: &HashMap<String, *mut std::ffi::c_void>,
    is_stub: bool,
) -> Handle {
    let name = if name_.ends_with(".so") {
        name_.to_string()
    } else {
        format!("lib{}.so", name_)
    };
    log::info!("linker: load_library_internal '{}' (is_stub={})", name, is_stub);
    let mut state = STATE.write().unwrap();

    if let Some(&handle) = state.libraries_by_name.get(name.as_str()) {
        if let Some(lib) = state.libraries_by_handle.get_mut(&handle) {
            lib.ref_count += 1;
            return handle;
        }
    }

    let handle = state.next_handle;
    state.next_handle += 1;

    for (k, v) in external_symbols {
        state.global_symbols.insert(k.clone(), *v as usize);
    }

    // For stub libraries, just register them
    if is_stub || name.starts_with("libdl") || name.starts_with("libstdc++") || name.starts_with("libOpenSLES") {
        let syms: HashMap<String, usize> = external_symbols
            .iter()
            .map(|(k, v)| (k.clone(), *v as usize))
            .collect();
        let soinfo = SoInfo {
            name: name.to_string(),
            soname: name.to_string(),
            is_stub: true,
            base: 0,
            size: 0,
            external_symbols: syms,
            ..Default::default()
        };
        let lib = LoadedLibrary {
            soinfo,
            ref_count: 1,
            is_stub: true,
            is_linked: true,
        };
        state.libraries_by_handle.insert(handle, lib);
        state.libraries_by_name.insert(name.to_string(), handle);
        return handle;
    }

    // Build search paths: registered paths first (treat as directories), then defaults
    let search_paths: Vec<String> = {
        let mut p: Vec<String> = state
            .search_paths
            .iter()
            .map(|dir| {
                let sep = if dir.ends_with('/') { "" } else { "/" };
                format!("{}{}{}", dir, sep, name)
            })
            .collect();
        p.push(format!("./{}", name));
        p.push(format!("./lib{}", name));
        p.push(format!("/usr/lib/{}", name));
        p.push(format!("/usr/lib/x86_64-linux-gnu/{}", name));
        p.push(format!("/lib/x86_64-linux-gnu/{}", name));
        p.push(format!("/usr/lib/lib{}", name));
        p.push(format!("/lib/lib{}", name));
        p
    };

    for path in &search_paths {
        log::debug!("linker: trying path: '{}'", path);
        if let Ok(data) = std::fs::read(path) {
            match loader::load_elf(&data, &name) {
                Ok(mut loaded) => {
                    log::info!("linker: found ELF at '{}'", path);
                    // Add external symbols
                    for (k, v) in external_symbols {
                        loaded
                            .soinfo
                            .external_symbols
                            .insert(k.clone(), *v as usize);
                    }

                    // Drop lock before loading deps (load_library_internal needs it)
                    drop(state);

                    // Recursively load DT_NEEDED dependencies
                    load_dependencies(&mut loaded.soinfo, &data, &name, external_symbols);

                    // Re-acquire lock for remainder
                    let mut state = STATE.write().unwrap();

                    // Make all segments writable for relocation
                    let prot = libc::PROT_READ | libc::PROT_WRITE;
                    unsafe {
                        libc::mprotect(
                            loaded.soinfo.base as *mut libc::c_void,
                            loaded.soinfo.size,
                            prot,
                        );
                    }

                    // Apply relocations
                    let resolve = |sym_name: &str| -> Option<usize> {
                        if let Some(&addr) = state.global_symbols.get(sym_name) {
                            return Some(addr);
                        }
                        if let Some(&addr) = loaded.soinfo.external_symbols.get(sym_name) {
                            return Some(addr);
                        }
                        // Search other loaded libs
                        for (_, lib) in &state.libraries_by_handle {
                            if lib.is_stub { continue; }
                            if let Some((addr, _)) = symbol::find_symbol(&lib.soinfo, sym_name) {
                                if addr != 0 { return Some(addr); }
                            }
                            if let Some(&addr) = lib.soinfo.external_symbols.get(sym_name) {
                                return Some(addr);
                            }
                        }
                        // Try C++ dlsym fallback for symbols managed by the C++ linker
                        if let Some(cpp_dlsym) = DLSYM_FALLBACK.get() {
                            let c_name = std::ffi::CString::new(sym_name).ok()?;
                            let addr = unsafe { cpp_dlsym(c_name.as_ptr()) };
                            if !addr.is_null() {
                                return Some(addr as usize);
                            }
                        }
                        // Note: resolve_symbol acquires STATE.read(), but state (write guard) is held — deadlock.
                        // The logic above already covers what resolve_symbol does, so fallback is None.
                        None
                    };

                    let has_reloc_errs = if let Err(errs) = reloc::apply_relocations(&loaded.soinfo, &resolve) {
                        let sym_count = errs.iter().filter(|e| matches!(e, reloc::RelocError::SymbolNotFound(_))).count();
                        let other_count = errs.len() - sym_count;
                        if sym_count > 0 {
                            log::warn!("linker: {} unresolved symbols in {} ({} other errors)", sym_count, name, other_count);
                        }
                        if other_count > 0 {
                            for e in &errs {
                                if !matches!(e, reloc::RelocError::SymbolNotFound(_)) {
                                    log::warn!("linker: relocation error for {}: {:?}", name, e);
                                }
                            }
                        }
                        !errs.is_empty()
                    } else {
                        false
                    };

                    // Restore segment protections per original LOAD flags
                    for &(seg_addr, seg_size, seg_prot) in &loaded.soinfo.load_segments {
                        unsafe {
                            libc::mprotect(
                                seg_addr as *mut libc::c_void,
                                seg_size,
                                seg_prot,
                            );
                        }
                    }

                    // Apply RELRO (make read-only after relocations) on top
                    if let Some((relro_addr, relro_size)) = loaded.soinfo.pt_gnu_relro {
                        unsafe {
                            libc::mprotect(
                                relro_addr as *mut libc::c_void,
                                relro_size,
                                libc::PROT_READ,
                            );
                        }
                    }

                    // Register TLS module if present
                    if let Some(ref tls_seg) = loaded.soinfo.tls_segment {
                        let tls_id = tls::register_tls_module(loaded.soinfo.base, tls_seg);
                        loaded.soinfo.tls_module_id = tls_id;
                        log::debug!(
                            "linker: registered TLS module id={} size={} align={} for '{}'",
                            tls_id,
                            tls_seg.size,
                            tls_seg.alignment,
                            name,
                        );
                    }

                    // Add all exported symbols to global_symbols for cross-library resolution
                    if let Some(symtab) = loaded.soinfo.symtab {
                        if let Some(strtab) = loaded.soinfo.strtab {
                            let base = loaded.soinfo.base;
                            let end = base + loaded.soinfo.size;
                            let mut i = 0usize;
                            loop {
                                let entry_addr = symtab.wrapping_add(i * 24);
                                if entry_addr >= end || entry_addr < symtab {
                                    break;
                                }
                                unsafe {
                                    let sym = *(entry_addr as *const crate::soinfo::Elf64_Sym);
                                    if sym.st_name == 0 && sym.st_shndx == 0 && sym.st_value == 0 && i > 0 {
                                        break;
                                    }
                                    if sym.st_name != 0 && (sym.st_name as usize) < loaded.soinfo.strtab_size {
                                        let name_ptr = (strtab as *const u8).add(sym.st_name as usize);
                                        let cstr = std::ffi::CStr::from_ptr(name_ptr as *const i8);
                                        if let Ok(s) = cstr.to_str() {
                                            let addr = base + sym.st_value as usize;
                                            if addr != 0 {
                                                state.global_symbols.entry(s.to_string()).or_insert(addr);
                                            }
                                        }
                                    }
                                }
                                i += 1;
                            }
                        }
                    }

                    loaded.soinfo.is_stub = false;

                    log::info!(
                        "linker: loaded ELF '{}' at {:x} size {}",
                        loaded.soinfo.soname,
                        loaded.soinfo.base,
                        loaded.soinfo.size,
                    );

                    // Call init functions only if no unresolved symbols
                    if !has_reloc_errs {
                        unsafe {
                            call_init_array(&loaded.soinfo);
                            call_init(&loaded.soinfo);
                        }
                    } else {
                        log::warn!("linker: skipping init for {} due to unresolved symbols", name);
                    }

                    let soname = loaded.soinfo.soname.clone();
                    let lib = LoadedLibrary {
                        soinfo: loaded.soinfo,
                        ref_count: 1,
                        is_stub: false,
                        is_linked: true,
                    };

                    state.libraries_by_handle.insert(handle, lib);
                    state.libraries_by_name.insert(soname.clone(), handle);
                    if soname != name {
                        state.libraries_by_name.insert(name.to_string(), handle);
                    }
                    return handle;
                }
                Err(e) => {
                    log::debug!("linker: failed to load {} from {}: {:?}", name, path, e);
                }
            }
        }
    }

    // Not found — register as stub
    log::warn!("linker: library '{}' not found on disk, registering as stub", name);
    let syms: HashMap<String, usize> = external_symbols
        .iter()
        .map(|(k, v)| (k.clone(), *v as usize))
        .collect();
    let soinfo = SoInfo {
        name: name.to_string(),
        soname: name.to_string(),
        is_stub: true,
        external_symbols: syms,
        ..Default::default()
    };
    let lib = LoadedLibrary {
        soinfo,
        ref_count: 1,
        is_stub: true,
        is_linked: true,
    };
    state.libraries_by_handle.insert(handle, lib);
    state.libraries_by_name.insert(name.to_string(), handle);
    log::debug!("linker: registered stub library '{}'", name);
    handle
}

unsafe fn call_init_array(soinfo: &SoInfo) {
    if let Some((addr, count)) = soinfo.init_array {
        let n = count / std::mem::size_of::<usize>();
        if n == 0 {
            return;
        }
        let arr = std::slice::from_raw_parts(addr as *const usize, n);
        for &f_addr in arr {
            if f_addr != 0 {
                let f: unsafe extern "C" fn() = std::mem::transmute(f_addr);
                f();
            }
        }
    }
}

unsafe fn call_init(soinfo: &SoInfo) {
    if let Some(addr) = soinfo.init {
        let f: unsafe extern "C" fn() = std::mem::transmute(addr);
        f();
    }
}

pub fn unload_library(handle: Handle) -> i32 {
    let mut state = STATE.write().unwrap();
    if let Some(lib) = state.libraries_by_handle.get_mut(&handle) {
        if lib.ref_count > 1 {
            lib.ref_count -= 1;
            return 0;
        }
        state.libraries_by_handle.remove(&handle);
        state.libraries_by_name.retain(|_, v| *v != handle);
        0
    } else {
        -1
    }
}

pub fn dlopen(path: &str, _flags: i32) -> Option<Handle> {
    let symbols = HashMap::new();
    Some(load_library_internal(path, &symbols, false))
}

/// Hook entry for dlopen_ext
#[repr(C)]
pub struct McpelauncherHook {
    pub name: *const std::ffi::c_char,
    pub value: *mut std::ffi::c_void,
}

pub fn dlopen_ext(
    path: &str,
    _flags: i32,
    hooks: &[McpelauncherHook],
) -> Option<Handle> {
    let mut symbols: HashMap<String, *mut std::ffi::c_void> = HashMap::new();
    for hook in hooks {
        if !hook.name.is_null() {
            let name = unsafe { std::ffi::CStr::from_ptr(hook.name).to_str().unwrap_or("") };
            symbols.insert(name.to_string(), hook.value);
        }
    }
    Some(load_library_internal(path, &symbols, false))
}

pub fn dlsym(handle: Handle, symbol: &str) -> Option<*mut std::ffi::c_void> {
    let state = STATE.read().unwrap();
    let lib = state.libraries_by_handle.get(&handle)?;

    if let Some(&addr) = lib.soinfo.external_symbols.get(symbol) {
        return Some(addr as *mut std::ffi::c_void);
    }
    if !lib.is_stub {
        if let Some((addr, _)) = symbol::find_symbol(&lib.soinfo, symbol) {
            if addr != 0 {
                return Some(addr as *mut std::ffi::c_void);
            }
        }
    }
    None
}

pub fn dlsym_global(symbol: &str) -> Option<*mut std::ffi::c_void> {
    let state = STATE.read().unwrap();
    if let Some(&addr) = state.global_symbols.get(symbol) {
        return Some(addr as *mut std::ffi::c_void);
    }
    for (_, lib) in &state.libraries_by_handle {
        if lib.is_stub {
            continue;
        }
        if let Some((addr, _)) = symbol::find_symbol(&lib.soinfo, symbol) {
            if addr != 0 {
                return Some(addr as *mut std::ffi::c_void);
            }
        }
        if let Some(&addr) = lib.soinfo.external_symbols.get(symbol) {
            return Some(addr as *mut std::ffi::c_void);
        }
    }
    None
}

pub fn dlclose(handle: Handle) -> i32 {
    unload_library(handle)
}

pub fn dlerror() -> String {
    String::new()
}

pub fn get_library_base(handle: Handle) -> usize {
    let state = STATE.read().unwrap();
    state
        .libraries_by_handle
        .get(&handle)
        .map(|l| l.soinfo.base)
        .unwrap_or(0)
}

pub fn get_library_code_region(handle: Handle) -> (usize, usize) {
    let state = STATE.read().unwrap();
    if let Some(lib) = state.libraries_by_handle.get(&handle) {
        (lib.soinfo.base, lib.soinfo.size)
    } else {
        (0, 0)
    }
}

#[no_mangle]
pub unsafe extern "C" fn linker_rust_get_library_code_region(
    handle: usize,
    base: *mut usize,
    size: *mut usize,
) {
    let (b, s) = get_library_code_region(handle);
    unsafe {
        if !base.is_null() { *base = b; }
        if !size.is_null() { *size = s; }
    }
}

pub fn add_symbols(handle: Handle, symbols: &HashMap<String, *mut std::ffi::c_void>) {
    let mut state = STATE.write().unwrap();
    if let Some(lib) = state.libraries_by_handle.get_mut(&handle) {
        for (k, v) in symbols {
            lib.soinfo
                .external_symbols
                .insert(k.clone(), *v as usize);
        }
    }
    for (k, v) in symbols {
        state.global_symbols.insert(k.clone(), *v as usize);
    }
}

pub fn dladdr(addr: *const std::ffi::c_void) -> Option<(Handle, String)> {
    let state = STATE.read().unwrap();
    let addr_val = addr as usize;
    for (&handle, lib) in &state.libraries_by_handle {
        let base = lib.soinfo.base;
        let size = lib.soinfo.size;
        if size > 0 && addr_val >= base && addr_val < base + size {
            return Some((handle, lib.soinfo.soname.clone()));
        }
    }
    None
}

pub fn show_state() {
    let state = STATE.read().unwrap();
    log::info!("linker: {} libraries loaded:", state.libraries_by_handle.len());
    for (handle, lib) in &state.libraries_by_handle {
        log::info!(
            "  handle={} name={} base=0x{:x} stub={} linked={} ref={}",
            handle, lib.soinfo.name, lib.soinfo.base, lib.is_stub, lib.is_linked, lib.ref_count
        );
    }
    log::info!("linker: {} global symbols", state.global_symbols.len());
}

// --- extern "C" exports for C++ link compatibility ---
// These are called from capi.cpp's C++-mangled linker wrappers.

/// Helper to convert parallel C arrays (keys, vals of length len) into a HashMap
unsafe fn c_arrays_to_hashmap(
    keys: *const *const libc::c_char,
    vals: *const *mut libc::c_void,
    len: usize,
) -> HashMap<String, *mut std::ffi::c_void> {
    let mut map = HashMap::new();
    if keys.is_null() || vals.is_null() {
        return map;
    }
    for i in 0..len {
        let k = unsafe { *keys.add(i) };
        let v = unsafe { *vals.add(i) };
        if !k.is_null() {
            if let Ok(s) = unsafe { std::ffi::CStr::from_ptr(k) }.to_str() {
                map.insert(s.to_string(), v as *mut std::ffi::c_void);
            }
        }
    }
    map
}

#[no_mangle]
pub unsafe extern "C" fn linker_init_rust() {
    // Initialize Rust linker state only
    init();
}

#[no_mangle]
pub unsafe extern "C" fn linker_load_library_rust(
    name: *const libc::c_char,
    keys: *const *const libc::c_char,
    vals: *const *mut libc::c_void,
    len: usize,
) -> usize {
    let name_str = unsafe { std::ffi::CStr::from_ptr(name) }.to_str().unwrap_or("");
    let map = unsafe { c_arrays_to_hashmap(keys, vals, len) };
    // Always register as stub — C++ linker::load_library() only creates stub
    // libraries, never real ELF loads (those go through linker::dlopen()).
    load_library_internal(name_str, &map, true)
}

#[no_mangle]
pub unsafe extern "C" fn linker_add_symbols_to_library_rust(
    name: *const libc::c_char,
    keys: *const *const libc::c_char,
    vals: *const *mut libc::c_void,
    len: usize,
) {
    let name_str = unsafe { std::ffi::CStr::from_ptr(name) }
        .to_str()
        .unwrap_or("");
    let map = unsafe { c_arrays_to_hashmap(keys, vals, len) };
    let handle = {
        let state = STATE.read().unwrap();
        state.libraries_by_name.get(name_str).copied()
    };
    if let Some(h) = handle {
        add_symbols(h, &map);
    }
}

#[no_mangle]
pub unsafe extern "C" fn linker_show_state_rust() {
    show_state();
}

#[no_mangle]
pub unsafe extern "C" fn linker_rust_set_dlsym_fallback(
    fallback: unsafe extern "C" fn(*const libc::c_char) -> *mut libc::c_void,
) {
    let _ = DLSYM_FALLBACK.set(fallback);
}

#[no_mangle]
pub unsafe extern "C" fn linker_rust_add_search_path(path: *const libc::c_char) {
    let s = unsafe { std::ffi::CStr::from_ptr(path) }
        .to_str()
        .unwrap_or("")
        .to_string();
    log::info!("linker: adding search path: '{}'", s);
    let mut state = STATE.write().unwrap();
    if !state.search_paths.contains(&s) {
        state.search_paths.push(s);
        log::info!("linker: search paths now: {:?}", state.search_paths);
    }
}

/// Try to load a library via the Rust linker (real ELF loading, not stub).
/// Returns a non-zero handle on success, 0 on failure (caller falls back to C++).
/// Hook names/vals are C arrays terminated by a null name entry.
#[no_mangle]
pub unsafe extern "C" fn linker_rust_dlopen_ext(
    filename: *const libc::c_char,
    _flags: i32,
    hook_names: *const *const libc::c_char,
    hook_vals: *const *mut libc::c_void,
    hook_count: usize,
) -> usize {
    let path = unsafe { std::ffi::CStr::from_ptr(filename) }
        .to_str()
        .unwrap_or("");
    if path.is_empty() {
        return 0;
    }

    let mut symbols: HashMap<String, *mut std::ffi::c_void> = HashMap::new();
    for i in 0..hook_count {
        if hook_names.is_null() || hook_vals.is_null() {
            break;
        }
        let name_ptr = unsafe { *hook_names.add(i) };
        let val_ptr = unsafe { *hook_vals.add(i) };
        if name_ptr.is_null() {
            break; // null name terminates
        }
        if let Ok(s) = unsafe { std::ffi::CStr::from_ptr(name_ptr) }.to_str() {
            symbols.insert(s.to_string(), val_ptr);
        }
    }

    log::info!("linker: Rust dlopen_ext attempting '{}' with {} hooks", path, symbols.len());

    // Call load_library_internal but skip constructors (safe for diagnostic trial)
    load_library_internal_no_ctors(path, &symbols, false)
}

/// C-exported dlsym for Rust-loaded libraries.
/// Returns the symbol address, or null if not found.
#[no_mangle]
pub unsafe extern "C" fn linker_rust_dlsym(
    handle: usize,
    symbol: *const libc::c_char,
) -> *mut libc::c_void {
    if handle == 0 || symbol.is_null() {
        return std::ptr::null_mut();
    }
    let sym_str = match unsafe { std::ffi::CStr::from_ptr(symbol) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    match dlsym(handle, sym_str) {
        Some(addr) => addr,
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn linker_rust_dlopen(
    name: *const libc::c_char,
    _flags: i32,
) -> usize {
    if name.is_null() {
        return 0;
    }
    let s = match std::ffi::CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return 0,
    };
    dlopen(s, _flags).unwrap_or(0)
}

#[no_mangle]
pub unsafe extern "C" fn linker_rust_dlclose(handle: usize) -> i32 {
    dlclose(handle)
}

#[no_mangle]
pub unsafe extern "C" fn linker_rust_get_library_base(handle: usize) -> usize {
    get_library_base(handle)
}

#[no_mangle]
pub unsafe extern "C" fn linker_rust_dlerror() -> *const libc::c_char {
    std::ptr::null()
}

/// Same as load_library_internal but skips calling init/init_array constructors.
/// Used by the diagnostic dlopen_ext to avoid double-initializing the game library.
fn load_library_internal_no_ctors(
    name_: &str,
    external_symbols: &HashMap<String, *mut std::ffi::c_void>,
    is_stub: bool,
) -> usize {
    let name = if name_.ends_with(".so") {
        name_.to_string()
    } else {
        format!("lib{}.so", name_)
    };
    log::info!("linker: load_library_internal_no_ctors '{}' (is_stub={})", name, is_stub);
    let mut state = STATE.write().unwrap();

    if let Some(&handle) = state.libraries_by_name.get(name.as_str()) {
        if let Some(lib) = state.libraries_by_handle.get_mut(&handle) {
            lib.ref_count += 1;
            return handle;
        }
    }

    let handle = state.next_handle;
    state.next_handle += 1;

    for (k, v) in external_symbols {
        state.global_symbols.insert(k.clone(), *v as usize);
    }

    // For stub libraries, just register them
    if is_stub || name.starts_with("libdl") || name.starts_with("libstdc++") || name.starts_with("libOpenSLES") {
        let syms: HashMap<String, usize> = external_symbols
            .iter()
            .map(|(k, v)| (k.clone(), *v as usize))
            .collect();
        let soinfo = SoInfo {
            name: name.to_string(),
            soname: name.to_string(),
            is_stub: true,
            base: 0,
            size: 0,
            external_symbols: syms,
            ..Default::default()
        };
        let lib = LoadedLibrary {
            soinfo,
            ref_count: 1,
            is_stub: true,
            is_linked: true,
        };
        state.libraries_by_handle.insert(handle, lib);
        state.libraries_by_name.insert(name.to_string(), handle);
        return handle;
    }

    // Build search paths: registered paths first (treat as directories), then defaults
    let search_paths: Vec<String> = {
        let mut p: Vec<String> = state
            .search_paths
            .iter()
            .map(|dir| {
                let sep = if dir.ends_with('/') { "" } else { "/" };
                format!("{}{}{}", dir, sep, name)
            })
            .collect();
        p.push(format!("./{}", name));
        p.push(format!("./lib{}", name));
        p.push(format!("/usr/lib/{}", name));
        p.push(format!("/usr/lib/x86_64-linux-gnu/{}", name));
        p.push(format!("/lib/x86_64-linux-gnu/{}", name));
        p.push(format!("/usr/lib/lib{}", name));
        p.push(format!("/lib/lib{}", name));
        p
    };

    'search: for path in &search_paths {
        log::debug!("linker: trying path: '{}'", path);
        let data = match std::fs::read(path) {
            Ok(d) => {
                log::info!("linker: read {} bytes from '{}'", d.len(), path);
                d
            }
            Err(e) => {
                log::info!("linker: failed to read {}: {:?}", path, e);
                continue;
            }
        };
        let mut loaded = match loader::load_elf(&data, &name) {
            Ok(l) => {
                log::info!("linker: found ELF at '{}' -> base=0x{:x} size={}", path, l.soinfo.base, l.soinfo.size);
                l
            }
            Err(e) => {
                log::debug!("linker: failed to load {} from {}: {:?}", name, path, e);
                continue;
            }
        };

        for (k, v) in external_symbols {
            loaded.soinfo.external_symbols.insert(k.clone(), *v as usize);
        }

        // Register exported symbols early so dependencies can resolve them
        let base = loaded.soinfo.base;
        let seg_end = base + loaded.soinfo.size;
        if let Some(symtab) = loaded.soinfo.symtab {
            if let Some(strtab) = loaded.soinfo.strtab {
                let mut i = 0usize;
                loop {
                    let entry_addr = symtab.wrapping_add(i * 24);
                    if entry_addr >= seg_end || entry_addr < symtab {
                        break;
                    }
                    unsafe {
                        let sym = *(entry_addr as *const crate::soinfo::Elf64_Sym);
                        if sym.st_name == 0 && sym.st_shndx == 0 && sym.st_value == 0 && i > 0 {
                            break;
                        }
                        if sym.st_name != 0 && (sym.st_name as usize) < loaded.soinfo.strtab_size {
                            let name_ptr = (strtab as *const u8).add(sym.st_name as usize);
                            let cstr = std::ffi::CStr::from_ptr(name_ptr as *const i8);
                            if let Ok(s) = cstr.to_str() {
                                let addr = base + sym.st_value as usize;
                                if addr != 0 {
                                    state.global_symbols.entry(s.to_string()).or_insert(addr);
                                }
                            }
                        }
                    }
                    i += 1;
                }
            }
        }

        // Drop lock before loading deps (load_library_internal needs it)
        drop(state);

        load_dependencies(&mut loaded.soinfo, &data, &name, external_symbols);

        // Re-acquire lock for relocation + registration
        let mut state = STATE.write().unwrap();

        // Make all segments writable for relocation
        unsafe {
            libc::mprotect(
                loaded.soinfo.base as *mut libc::c_void,
                loaded.soinfo.size,
                libc::PROT_READ | libc::PROT_WRITE,
            );
        }
        // Resolver: global symbols → external hooks → loaded libs → C++ dlsym → builtin
        let resolve = |sym_name: &str| -> Option<usize> {
            if let Some(&addr) = state.global_symbols.get(sym_name) {
                return Some(addr);
            }
            if let Some(&addr) = loaded.soinfo.external_symbols.get(sym_name) {
                return Some(addr);
            }
            for (_, lib) in &state.libraries_by_handle {
                if lib.is_stub { continue; }
                if let Some((addr, _)) = symbol::find_symbol(&lib.soinfo, sym_name) {
                    if addr != 0 { return Some(addr); }
                }
                if let Some(&addr) = lib.soinfo.external_symbols.get(sym_name) {
                    return Some(addr);
                }
            }
            if let Some(cpp_dlsym) = DLSYM_FALLBACK.get() {
                let c_name = std::ffi::CString::new(sym_name).ok()?;
                let addr = unsafe { cpp_dlsym(c_name.as_ptr()) };
                if !addr.is_null() {
                    return Some(addr as usize);
                }
            }
            // Note: resolve_symbol acquires STATE.read(), but state (write guard) is held — deadlock.
            // The logic above already covers what resolve_symbol does, so fallback is None.
            None
        };

        if let Err(errs) = reloc::apply_relocations(&loaded.soinfo, &resolve) {
            let sym_count = errs.iter().filter(|e| matches!(e, reloc::RelocError::SymbolNotFound(_))).count();
            let other_count = errs.len() - sym_count;
            if sym_count > 0 {
                log::warn!("linker: {} unresolved symbols in {} ({} other errors)", sym_count, name, other_count);
            }
            if other_count > 0 {
                for e in &errs {
                    if !matches!(e, reloc::RelocError::SymbolNotFound(_)) {
                        log::warn!("linker: relocation error for {}: {:?}", name, e);
                    }
                }
            }
        }
        // Apply RELRO (make read-only after relocations)
        if let Some((relro_addr, relro_size)) = loaded.soinfo.pt_gnu_relro {
            unsafe {
                libc::mprotect(relro_addr as *mut libc::c_void, relro_size, libc::PROT_READ);
            }
        }

        // Restore segment protections for executable segments
        unsafe {
            libc::mprotect(
                loaded.soinfo.base as *mut libc::c_void,
                loaded.soinfo.size,
                libc::PROT_READ | libc::PROT_EXEC,
            );
        }

        // Register TLS module if present
        if let Some(ref tls_seg) = loaded.soinfo.tls_segment {
            let tls_id = tls::register_tls_module(loaded.soinfo.base, tls_seg);
            loaded.soinfo.tls_module_id = tls_id;
            log::debug!(
                "linker: registered TLS module id={} size={} align={} for '{}'",
                tls_id, tls_seg.size, tls_seg.alignment, name,
            );
        }

        // Add all exported symbols to global_symbols for cross-library resolution
        if let Some(symtab) = loaded.soinfo.symtab {
            if let Some(strtab) = loaded.soinfo.strtab {
                // Iterate until we go past the mapped segment to avoid reading garbage
                let base = loaded.soinfo.base;
                let end = base + loaded.soinfo.size;
                let mut i = 0;
                loop {
                    let entry_addr = symtab.wrapping_add(i * 24);
                    if entry_addr >= end || entry_addr < symtab {
                        break;
                    }
                    unsafe {
                        let sym = *(entry_addr as *const crate::soinfo::Elf64_Sym);
                        if sym.st_name == 0 && sym.st_shndx == 0 && sym.st_value == 0 && i > 0 {
                            break;
                        }
                        if sym.st_name != 0 && (sym.st_name as usize) < loaded.soinfo.strtab_size {
                            let name_ptr = (strtab as *const u8).add(sym.st_name as usize);
                            let cstr = std::ffi::CStr::from_ptr(name_ptr as *const i8);
                            if let Ok(s) = cstr.to_str() {
                                let addr = base + sym.st_value as usize;
                                if addr != 0 {
                                    state.global_symbols.entry(s.to_string()).or_insert(addr);
                                }
                            }
                        }
                    }
                    i += 1;
                }
            }
        }

        loaded.soinfo.is_stub = false;

        log::info!(
            "linker: (no-ctors) loaded ELF '{}' at {:x} size {}",
            loaded.soinfo.soname,
            loaded.soinfo.base,
            loaded.soinfo.size,
        );

        let soname = loaded.soinfo.soname.clone();
        let lib = LoadedLibrary {
            soinfo: loaded.soinfo,
            ref_count: 1,
            is_stub: false,
            is_linked: true,
        };

        state.libraries_by_handle.insert(handle, lib);
        state.libraries_by_name.insert(soname.clone(), handle);
        if soname != name {
            state.libraries_by_name.insert(name.to_string(), handle);
        }
        return handle;
    }

    log::warn!("linker: library '{}' not found on disk, registering as stub", name);
    let syms: HashMap<String, usize> = external_symbols
        .iter()
        .map(|(k, v)| (k.clone(), *v as usize))
        .collect();
    let soinfo = SoInfo {
        name: name.to_string(),
        soname: name.to_string(),
        is_stub: true,
        external_symbols: syms,
        ..Default::default()
    };
    let lib = LoadedLibrary {
        soinfo,
        ref_count: 1,
        is_stub: true,
        is_linked: true,
    };
    state.libraries_by_handle.insert(handle, lib);
    state.libraries_by_name.insert(name.to_string(), handle);
    log::debug!("linker: registered stub library '{}'", name);
    handle
}

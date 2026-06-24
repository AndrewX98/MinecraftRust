pub mod soinfo;
pub mod loader;
pub mod reloc;
pub mod symbol;

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
}

impl LinkerState {
    fn new() -> Self {
        Self {
            libraries_by_handle: HashMap::new(),
            libraries_by_name: HashMap::new(),
            global_symbols: HashMap::new(),
            next_handle: 1,
        }
    }
}

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
    log::info!("linker: initializing");
    *STATE.write().unwrap() = LinkerState::new();
    let mut dl_syms: HashMap<String, *mut std::ffi::c_void> = HashMap::new();
    dl_syms.insert("dlopen".to_string(), libc::dlopen as *mut std::ffi::c_void);
    dl_syms.insert("dlsym".to_string(), libc::dlsym as *mut std::ffi::c_void);
    dl_syms.insert("dlclose".to_string(), libc::dlclose as *mut std::ffi::c_void);
    dl_syms.insert("dlerror".to_string(), libc::dlerror as *mut std::ffi::c_void);
    load_library_internal("libdl.so", &dl_syms, true);
    log::info!("linker: initialized");
}

fn load_dependencies(
    soinfo: &mut SoInfo,
    data: &[u8],
    name: &str,
    external_symbols: &HashMap<String, *mut std::ffi::c_void>,
) {
    let deps = soinfo.dependencies.clone();
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

    // Search for the ELF file
    let search_paths: Vec<String> = {
        let mut p = Vec::new();
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
        if let Ok(data) = std::fs::read(path) {
            match loader::load_elf(&data, &name) {
                Ok(mut loaded) => {
                    // Add external symbols
                    for (k, v) in external_symbols {
                        loaded
                            .soinfo
                            .external_symbols
                            .insert(k.clone(), *v as usize);
                    }

                    // Recursively load DT_NEEDED dependencies
                    load_dependencies(&mut loaded.soinfo, &data, &name, external_symbols);

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
                        resolve_symbol(sym_name)
                    };

                    if let Err(errs) = reloc::apply_relocations(&loaded.soinfo, &resolve) {
                        for e in &errs {
                            log::warn!("linker: relocation error for {}: {:?}", name, e);
                        }
                    }

                    // Restore segment protections
                    unsafe {
                        libc::mprotect(
                            loaded.soinfo.base as *mut libc::c_void,
                            loaded.soinfo.size,
                            libc::PROT_READ | libc::PROT_EXEC,
                        );
                    }

                    // Add all exported symbols to global_symbols for cross-library resolution
                    if let Some(symtab) = loaded.soinfo.symtab {
                        if let Some(strtab) = loaded.soinfo.strtab {
                            let sym_count = loaded.soinfo.strtab_size / 24;
                            for i in 0..sym_count {
                                unsafe {
                                    let sym_ptr = (symtab as *const u8).add(i * 24) as *const goblin::elf::Sym;
                                    let sym = *sym_ptr;
                                    if sym.st_name != 0 && sym.st_shndx != 0 && sym.st_value != 0 {
                                        let name_ptr = (strtab as *const u8).add(sym.st_name as usize);
                                        let cstr = std::ffi::CStr::from_ptr(name_ptr as *const i8);
                                        if let Ok(s) = cstr.to_str() {
                                            let addr = loaded.soinfo.base + sym.st_value as usize;
                                            state.global_symbols.entry(s.to_string()).or_insert(addr);
                                        }
                                    }
                                }
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

                    // Call init functions
                    unsafe {
                        call_init_array(&loaded.soinfo);
                        call_init(&loaded.soinfo);
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
    load_library(name_str, &map)
}

#[no_mangle]
pub unsafe extern "C" fn linker_show_state_rust() {
    show_state();
}

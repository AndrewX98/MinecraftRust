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

/// Libraries the C++ bionic linker already loads before Rust loads
/// `libminecraftpe.so` (see `MinecraftUtils::loadMinecraftLib`).
///
/// Re-mapping a second copy in the Rust linker is catastrophic: the second
/// image cannot resolve its libc imports cleanly (thousands of unresolved
/// JUMP_SLOTs), so its constructors and C++ runtime are half-broken. Game
/// DT_INIT_ARRAY then jumps into that broken image (e.g. `shared_timed_mutex`
/// ctor → SIGSEGV at a raw ELF offset like `0x12a3e6`).
///
/// Skip re-load; symbol lookup falls through to `DLSYM_FALLBACK` (C++ bionic
/// `RTLD_DEFAULT`), which returns addresses in the healthy first image.
fn is_cpp_preloaded_dependency(name: &str) -> bool {
    matches!(
        name,
        "libc++_shared.so"
            | "libfmod.so"
            | "libpairipcore.so"
            | "libsqliteX.so"
            | "libsqlite3.so"
    )
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
        if is_cpp_preloaded_dependency(dep_name) {
            log::info!(
                "linker: skipping dependency '{}' for '{}' (already loaded by C++ bionic linker)",
                dep_name,
                name
            );
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

/// Register defined GLOBAL/WEAK non-TLS symbols into `global_symbols`.
///
/// Must NOT register `SHN_UNDEF` imports: those have `st_value == 0`, so
/// `base + 0 == base` would poison the table. That caused JUMP_SLOT entries
/// (e.g. `__cxa_guard_acquire`) to point at the ELF header and SIGSEGV when
/// DT_INIT_ARRAY constructors ran.
fn register_global_exports(state: &mut LinkerState, soinfo: &SoInfo) {
    const SHN_UNDEF: u16 = 0;
    const STB_GLOBAL: u8 = 1;
    const STB_WEAK: u8 = 2;
    const STT_TLS: u8 = 6;

    let (Some(symtab), Some(strtab)) = (soinfo.symtab, soinfo.strtab) else {
        return;
    };
    let base = soinfo.base;
    let end = base + soinfo.size;
    let mut i = 0usize;
    loop {
        let entry_addr = symtab.wrapping_add(i * 24);
        if entry_addr >= end || entry_addr < symtab {
            break;
        }
        unsafe {
            let sym = *(entry_addr as *const crate::soinfo::Elf64_Sym);
            // Heuristic end of symbol table (null-ish entry).
            if sym.st_name == 0 && sym.st_shndx == 0 && sym.st_value == 0 && i > 0 {
                break;
            }
            if sym.st_name == 0 || (sym.st_name as usize) >= soinfo.strtab_size {
                i += 1;
                continue;
            }
            // Skip undefined imports — never publish base+0 as a symbol address.
            if sym.st_shndx == SHN_UNDEF {
                i += 1;
                continue;
            }
            let bind = sym.st_info >> 4;
            let typ = sym.st_info & 0xf;
            if typ == STT_TLS || (bind != STB_GLOBAL && bind != STB_WEAK) {
                i += 1;
                continue;
            }
            let name_ptr = (strtab as *const u8).add(sym.st_name as usize);
            let cstr = std::ffi::CStr::from_ptr(name_ptr as *const i8);
            if let Ok(s) = cstr.to_str() {
                if !s.is_empty() {
                    let addr = base.wrapping_add(sym.st_value as usize);
                    state.global_symbols.entry(s.to_string()).or_insert(addr);
                }
            }
        }
        i += 1;
    }
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

                    // Apply relocations.
                    // mcpelauncher hooks (external_symbols) must beat self-exports so
                    // JUMP_SLOTs for defined symbols like SwappyGL_* bind to stubs —
                    // matching C++ bionic's si_->symbols override. reloc::resolve_sym
                    // also checks external first; keep get_symbol consistent.
                    let resolve = |sym_name: &str| -> Option<usize> {
                        if let Some(&addr) = loaded.soinfo.external_symbols.get(sym_name) {
                            return Some(addr);
                        }
                        if let Some((addr, _)) = symbol::find_symbol(&loaded.soinfo, sym_name) {
                            if addr != 0 {
                                return Some(addr);
                            }
                        }
                        if let Some(&addr) = state.global_symbols.get(sym_name) {
                            return Some(addr);
                        }
                        // Search other loaded libs (include stubs: their symbols
                        // live in external_symbols / were also published to
                        // global_symbols at registration time).
                        for (_, lib) in &state.libraries_by_handle {
                            if let Some((addr, _)) = symbol::find_symbol(&lib.soinfo, sym_name) {
                                if addr != 0 {
                                    return Some(addr);
                                }
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

                    // Restore segment protections per original LOAD flags.
                    // Use page-aligned addresses/sizes to match kernel mprotect requirements.
                    const PAGE_MASK: usize = 0xfff;
                    const PAGE_SIZE: usize = 0x1000;
                    for &(seg_addr, seg_size, seg_prot) in &loaded.soinfo.load_segments {
                        let aligned_start = seg_addr & !PAGE_MASK;
                        let end = seg_addr + seg_size;
                        let aligned_end = (end + PAGE_MASK) & !PAGE_MASK;
                        let aligned_len = aligned_end - aligned_start;
                        unsafe {
                            libc::mprotect(
                                aligned_start as *mut libc::c_void,
                                aligned_len,
                                seg_prot,
                            );
                        }
                    }

                    // Save RELRO range -- applied AFTER constructors run (below)
                    let relro = loaded.soinfo.pt_gnu_relro;

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

                    // Add defined exports only (never SHN_UNDEF — see register_global_exports).
                    register_global_exports(&mut state, &loaded.soinfo);

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

                    // Apply RELRO after constructors have finished (they may write GOT entries)
                    if let Some((relro_addr, relro_size)) = relro {
                        let aligned_start = relro_addr & !PAGE_MASK;
                        let end = relro_addr + relro_size;
                        let aligned_end = (end + PAGE_MASK) & !PAGE_MASK;
                        let aligned_len = aligned_end - aligned_start;
                        unsafe {
                            libc::mprotect(
                                aligned_start as *mut libc::c_void,
                                aligned_len,
                                libc::PROT_READ,
                            );
                        }
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

    // Load the library (no constructors run yet)
    let rust_handle = load_library_internal_no_ctors(path, &symbols, false);
    if rust_handle == 0 {
        return 0;
    }

    // NOTE: We skip calling init functions (DT_INIT and DT_INIT_ARRAY) for
    // libminecraftpe.so because its constructors require a JNI environment
    // that isn't available at this point in the startup sequence. The game
    // will lazily initialize its global state when needed. The C++ bionic
    // linker also marks constructors as called without running them when
    // registering Rust-loaded libraries.

    // Register with C++ bionic linker so C++ APIs (HookManager, linker::dlsym) work.
    // Skip stub libraries — they are pre-registered by the C++ linker already.
    let (is_stub, base) = {
        let state = STATE.read().unwrap();
        match state.libraries_by_handle.get(&rust_handle) {
            None => (true, 0),
            Some(lib) => (lib.is_stub, lib.soinfo.base),
        }
    };

    if !is_stub && base != 0 {
        extern "C" {
            fn mcpelauncher_linker_register_loaded_library(
                name: *const libc::c_char,
                base: usize,
                rust_handle: usize,
            ) -> usize;
        }
        let cpp_handle =
            mcpelauncher_linker_register_loaded_library(filename, base, rust_handle);
        if cpp_handle != 0 {
            return cpp_handle; // C++ handle — all C++ APIs work natively
        }
    }

    // Return 0 if stub or registration failed — C++ side falls back to C++ linker.
    0
}

/// Symbol-lookup data exported from Rust SoInfo to C++ for direct soinfo
/// field population (bypasses prelink_image for Rust-loaded ELFs).
#[repr(C)]
pub struct SoInfoSymbolData {
    strtab: *const u8,
    strtab_size: usize,
    symtab: *const u8,
    // GNU hash fields
    gnu_nbucket: usize,
    gnu_bucket: *const u32,
    gnu_chain: *const u32,
    gnu_maskwords: u32,
    gnu_shift2: u32,
    gnu_bloom_filter: *const usize,
    // SysV hash fields
    nbucket: usize,
    nchain: usize,
    bucket: *const u32,
    chain: *const u32,
    has_gnu_hash: bool,
}

/// Populates a C struct with symbol-lookup data from the Rust SoInfo for
/// the library identified by `handle`. Returns false if handle is invalid.
#[no_mangle]
pub unsafe extern "C" fn linker_rust_get_soinfo_symbol_data(
    handle: usize,
    data: *mut SoInfoSymbolData,
) -> bool {
    if data.is_null() {
        return false;
    }
    let state = match STATE.read() {
        Ok(s) => s,
        Err(_) => return false,
    };
    let si = match state.libraries_by_handle.get(&handle) {
        Some(l) => &l.soinfo,
        None => return false,
    };
    let symdata = SoInfoSymbolData {
        strtab: si.strtab.map_or(std::ptr::null(), |a| a as *const u8),
        strtab_size: si.strtab_size,
        symtab: si.symtab.map_or(std::ptr::null(), |a| a as *const u8),
        has_gnu_hash: si.gnu_hash.is_some(),
        gnu_nbucket: si.gnu_bucket.len(),
        gnu_bucket: si.gnu_bucket.as_ptr(),
        gnu_chain: si.gnu_chain.as_ptr(),
        gnu_maskwords: si.gnu_bloom_n as u32,
        gnu_shift2: si.gnu_bloom_shift as u32,
        gnu_bloom_filter: si.gnu_bloom_filter.as_ptr(),
        nbucket: si.bucket.len(),
        nchain: si.chain.len(),
        bucket: si.bucket.as_ptr(),
        chain: si.chain.as_ptr(),
    };
    *data = symdata;
    true
}

/// Calls DT_INIT and DT_INIT_ARRAY constructors for a Rust-loaded library
/// identified by name. Temporarily makes RELRO writable so constructors
/// can update GOT entries, then re-applies RELRO read-only.
#[no_mangle]
pub unsafe extern "C" fn linker_rust_call_init_functions(name: *const libc::c_char) -> bool {
    let Ok(name_str) = unsafe { std::ffi::CStr::from_ptr(name) }.to_str() else {
        return false;
    };
    let name = if name_str.ends_with(".so") {
        name_str.to_string()
    } else {
        format!("lib{}.so", name_str)
    };
    let (has_init, has_init_array, relro_info) = {
        let state = match STATE.read() {
            Ok(s) => s,
            Err(_) => return false,
        };
        let Some(handle) = state.libraries_by_name.get(name.as_str()) else {
            log::warn!("linker: library '{}' not found for init functions", name);
            return false;
        };
        let Some(lib) = state.libraries_by_handle.get(handle) else {
            return false;
        };
        let has_init = lib.soinfo.init.is_some();
        let has_init_array = lib.soinfo.init_array.is_some();
        let relro_info = lib.soinfo.pt_gnu_relro;
        (has_init, has_init_array, relro_info)
    };
    if !has_init && !has_init_array {
        log::info!("linker: no init functions for '{}'", name);
        return true;
    }
    // Temporarily make RELRO writable — the no-ctors loader applied RELRO
    // before constructors ran, but constructors may write GOT entries.
    if let Some((relro_addr, relro_size)) = relro_info {
        const PAGE_SIZE: usize = 0x1000;
        let aligned_start = relro_addr & !(PAGE_SIZE - 1);
        let offset = relro_addr - aligned_start;
        let aligned_size = ((relro_size + offset + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)).max(PAGE_SIZE);
        unsafe {
            libc::mprotect(aligned_start as *mut libc::c_void, aligned_size,
                           libc::PROT_READ | libc::PROT_WRITE);
        }
        log::info!("linker: made RELRO writable for '{}' (addr={:#x} size={})",
                   name, relro_addr, relro_size);
    }
    // Call constructors with RELRO writable
    log::info!("linker: calling init functions for '{}'", name);
    let (init_addr, init_array_len) = {
        let state = STATE.read().unwrap();
        if let Some(handle) = state.libraries_by_name.get(name.as_str()) {
            if let Some(lib) = state.libraries_by_handle.get(handle) {
                let init_addr = lib.soinfo.init;
                let init_array_len = lib.soinfo.init_array.map(|(_, sz)| sz);
                unsafe { lib.soinfo.call_init_functions(); }
                (init_addr, init_array_len)
            } else {
                (None, None)
            }
        } else {
            (None, None)
        }
    };
    log::info!("linker: init functions done for '{}' (init={:#x?}, init_array_size={:?})",
               name, init_addr, init_array_len);
    // Re-apply RELRO read-only
    if let Some((relro_addr, relro_size)) = relro_info {
        const PAGE_SIZE: usize = 0x1000;
        let aligned_start = relro_addr & !(PAGE_SIZE - 1);
        let offset = relro_addr - aligned_start;
        let aligned_size = ((relro_size + offset + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)).max(PAGE_SIZE);
        unsafe {
            libc::mprotect(aligned_start as *mut libc::c_void, aligned_size, libc::PROT_READ);
        }
        log::info!("linker: re-applied RELRO read-only for '{}'", name);
    }
    true
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

        // Register defined exports early so dependencies can resolve them.
        // Must not publish SHN_UNDEF imports as base+0 (see register_global_exports).
        register_global_exports(&mut state, &loaded.soinfo);

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
        // Resolver: hooks → self → global → other libs → C++ dlsym
        // Hooks override defined exports (SwappyGL_*, AppPlatform mouse, …).
        let resolve = |sym_name: &str| -> Option<usize> {
            if let Some(&addr) = loaded.soinfo.external_symbols.get(sym_name) {
                return Some(addr);
            }
            if let Some((addr, _)) = symbol::find_symbol(&loaded.soinfo, sym_name) {
                if addr != 0 {
                    return Some(addr);
                }
            }
            if let Some(&addr) = state.global_symbols.get(sym_name) {
                return Some(addr);
            }
            // Include stubs: their exports are in external_symbols (and mirrored
            // into global_symbols at registration). Skipping is_stub used to
            // leave JUMP_SLOTs unbound when a stub was the only provider.
            for (_, lib) in &state.libraries_by_handle {
                if let Some((addr, _)) = symbol::find_symbol(&lib.soinfo, sym_name) {
                    if addr != 0 {
                        return Some(addr);
                    }
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
                // Log individual names for small failure sets (game lib is ~18 when healthy).
                if sym_count <= 64 {
                    for e in &errs {
                        if let reloc::RelocError::SymbolNotFound(sym) = e {
                            log::warn!("linker: unresolved symbol in {}: {}", name, sym);
                        }
                    }
                }
            }
            if other_count > 0 {
                for e in &errs {
                    if !matches!(e, reloc::RelocError::SymbolNotFound(_)) {
                        log::warn!("linker: relocation error for {}: {:?}", name, e);
                    }
                }
            }
        }
        // Restore per-segment protections (text -> r-x, data -> rw-, etc.)
        // mprotect(2) requires addr to be page-aligned, so align each segment.
        const PAGE_SIZE: usize = 0x1000;
        for &(seg_addr, seg_size, seg_prot) in &loaded.soinfo.load_segments {
            let aligned_start = seg_addr & !(PAGE_SIZE - 1);
            let offset = seg_addr - aligned_start;
            let aligned_size = ((seg_size + offset + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)).max(PAGE_SIZE);
            unsafe {
                libc::mprotect(aligned_start as *mut libc::c_void, aligned_size, seg_prot);
            }
        }

        // Apply RELRO on top of segment protections (limits RELRO range to read-only)
        if let Some((relro_addr, relro_size)) = loaded.soinfo.pt_gnu_relro {
            let relro_aligned_start = relro_addr & !(PAGE_SIZE - 1);
            let relro_offset = relro_addr - relro_aligned_start;
            let relro_aligned_size = ((relro_size + relro_offset + PAGE_SIZE - 1) & !(PAGE_SIZE - 1)).max(PAGE_SIZE);
            unsafe {
                libc::mprotect(relro_aligned_start as *mut libc::c_void, relro_aligned_size, libc::PROT_READ);
            }
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

        // Re-publish defined exports after relocations (deps may have filled more).
        register_global_exports(&mut state, &loaded.soinfo);

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

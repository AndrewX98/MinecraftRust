//! Port of `mcpelauncher-core/src/minecraft_utils.cpp` (Phase 6).
//!
//! The mod-facing `getApi`/`setupApi` funnel and the boot-adjacent helpers move
//! here. `mcpelauncher_log/vlog` (C varargs) and the Google-credentials helper
//! moved to Rust once the client crate enabled nightly `c_variadic`
//! (`client/src/mod_api.rs`); `jnivm_register_method` cannot be expressed in
//! Rust (it binds `jnivm::Method` native handles on the C++ FakeJni VM) so it
//! is stubbed in `mod_api.rs`. All are referenced here by extern "C" address.
//!
//! Clean-named `#[no_mangle]` twins are what `capi.cpp` now calls; the C++
//! `_ZN14MinecraftUtils*` mangled methods are gone (deleted with
//! `minecraft_utils.cpp`).

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::{Mutex, OnceLock};

/// Matches `shim::shimmed_symbol` in `crates/libc-shim` / `libc_shim` crate.
#[repr(C)]
pub struct ShimmedSymbol {
    pub name: *const c_char,
    pub value: *mut c_void,
}

/// glibc/bionic `Dl_info` (matches the `linker` crate mirror).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct DlInfo {
    pub dli_fname: *const c_char,
    pub dli_fbase: *mut c_void,
    pub dli_sname: *const c_char,
    pub dli_saddr: *mut c_void,
}

/// `{ name, hook }` used by `mcpelauncher_relocate2` / `mcpelauncher_load_library`.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct HookEntry {
    pub name: *const c_char,
    pub hook: *mut c_void,
}

extern "C" {
    // libc-shim (Rust)
    fn get_shimmed_symbols_len() -> usize;
    fn get_shimmed_symbols_fill(buf: *mut ShimmedSymbol);
    // linker dispatch
    fn linker_load_library_rust(
        name: *const c_char,
        keys: *const *const c_char,
        vals: *const *mut c_void,
        len: usize,
    ) -> usize;
    fn mcpelauncher_dispatch_dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn mcpelauncher_dispatch_dlopen(name: *const c_char, flags: i32) -> *mut c_void;
    fn mcpelauncher_dispatch_dlclose(handle: *mut c_void) -> i32;
    fn mcpelauncher_dispatch_dladdr(addr: *const c_void, info: *mut DlInfo) -> i32;
    fn mcpelauncher_dispatch_relocate(
        handle: *mut c_void,
        keys: *const *const c_char,
        vals: *const *mut c_void,
        len: usize,
    );
    fn mcpelauncher_dispatch_unload_library(handle: *mut c_void) -> i32;
    // client path_helper (Rust)
    fn path_helper_find_data_file(path: *const c_char) -> *const c_char;
    // client mod_api.rs (Rust)
    fn mc_mod_log(level: i32, tag: *const c_char, fmt: *const c_char);
    fn mc_mod_vlog(level: i32, tag: *const c_char, fmt: *const c_char);
    fn mc_mod_request_google_credentials(
        onsuccess: *const c_void,
        onfailure: *const c_void,
    );
    fn mc_mod_jnivm_register_method(
        env: *mut c_void,
        cl: *mut c_void,
        ty: i32,
        name: *const c_char,
        signature: *const c_char,
        cbk: *mut c_void,
    ) -> bool;
}

/// MCPE symbol-name tables the C++ fed to `HybrisUtils::loadLibraryOS`
/// (mirrors `libm_symbols.h` / `libz_symbols.h`).
const LIB_M_SYMS: &[&[u8]] = &[
    b"acos", b"acosf", b"acosh", b"acoshf", b"acoshl", b"acosl", b"asin", b"asinf",
    b"asinh", b"asinhf", b"asinhl", b"asinl", b"atan", b"atan2", b"atan2f", b"atan2l",
    b"atanf", b"atanh", b"atanhf", b"atanhl", b"atanl", b"cabsl", b"cbrt", b"cbrtf",
    b"cbrtl", b"ceil", b"ceilf", b"ceill", b"copysign", b"copysignf", b"copysignl",
    b"cos", b"cosf", b"cosh", b"coshf", b"coshl", b"cosl", b"cprojl", b"csqrtl",
    b"drem", b"dremf", b"erf", b"erfc", b"erfcf", b"erfcl", b"erff", b"erfl", b"exp",
    b"exp2", b"exp2f", b"exp2l", b"expf", b"expl", b"expm1", b"expm1f", b"expm1l",
    b"fabs", b"fabsf", b"fabsl", b"fdim", b"fdimf", b"fdiml", b"feclearexcept",
    b"fedisableexcept", b"feenableexcept", b"fegetenv", b"fegetexcept",
    b"fegetexceptflag", b"fegetround", b"feholdexcept", b"feraiseexcept",
    b"fesetenv", b"fesetexceptflag", b"fesetround", b"fetestexcept", b"feupdateenv",
    b"finite", b"finitef", b"floor", b"floorf", b"floorl", b"fma", b"fmaf", b"fmal",
    b"fmax", b"fmaxf", b"fmaxl", b"fmin", b"fminf", b"fminl", b"fmod", b"fmodf",
    b"fmodl", b"frexp", b"frexpf", b"frexpl", b"gamma", b"gammaf", b"gammaf_r",
    b"gamma_r", b"hypot", b"hypotf", b"hypotl", b"ilogb", b"ilogbf", b"ilogbl",
    b"j0", b"j0f", b"j1", b"j1f", b"jn", b"jnf", b"ldexpf", b"ldexpl", b"lgamma",
    b"lgammaf", b"lgammaf_r", b"lgammal", b"lgammal_r", b"lgamma_r", b"lldiv",
    b"llrint", b"llrintf", b"llrintl", b"llround", b"llroundf", b"llroundl",
    b"log", b"log10", b"log10f", b"log10l", b"log1p", b"log1pf", b"log1pl", b"log2",
    b"log2f", b"log2l", b"logb", b"logbf", b"logbl", b"logf", b"logl", b"lrint",
    b"lrintf", b"lrintl", b"lround", b"lroundf", b"lroundl", b"modf", b"modff",
    b"modfl", b"nan", b"nanf", b"nanl", b"nearbyint", b"nearbyintf", b"nearbyintl",
    b"nextafter", b"nextafterf", b"nextafterl", b"nexttoward", b"nexttowardf",
    b"nexttowardl", b"pow", b"powf", b"powl", b"remainder", b"remainderf",
    b"remainderl", b"remquo", b"remquof", b"remquol", b"rint", b"rintf", b"rintl",
    b"round", b"roundf", b"roundl", b"scalb", b"scalbf", b"scalbln", b"scalblnf",
    b"scalblnl", b"scalbn", b"scalbnf", b"scalbnl", b"__signbit", b"__signbitf",
    b"__signbitl", b"signgam", b"significand", b"significandf", b"significandl",
    b"sin", b"sincos", b"sincosf", b"sincosl", b"sinf", b"sinh", b"sinhf", b"sinhl",
    b"sinl", b"sqrt", b"sqrtf", b"sqrtl", b"tan", b"tanf", b"tanh", b"tanhf",
    b"tanhl", b"tanl", b"tgamma", b"tgammaf", b"tgammal", b"trunc", b"truncf",
    b"truncl", b"y0", b"y0f", b"y1", b"y1f", b"yn", b"ynf", b"isnan", b"isinf",
];
const LIB_Z_SYMS: &[&[u8]] = &[
    b"deflate", b"deflateEnd", b"deflateInit_", b"deflateInit2_", b"inflate",
    b"inflateEnd", b"inflateInit_", b"inflateInit2_", b"compressBound", b"crc32",
];

/// Build the owned CStrings + a null-terminated contiguous `*const c_char`
/// array (the actual C layout) for `mc_hybris_load_library_os`. The two Vecs
/// must outlive the call; callers keep them bound for the call expression.
fn names_ptrs(syms: &[&[u8]]) -> Option<(Vec<CString>, Vec<*const c_char>)> {
    let mut owned: Vec<CString> = Vec::with_capacity(syms.len());
    for s in syms {
        match CString::new(*s) {
            Ok(c) => owned.push(c),
            Err(_) => continue,
        }
    }
    if owned.is_empty() {
        return None;
    }
    let mut ptrs: Vec<*const c_char> = owned.iter().map(|c| c.as_ptr()).collect();
    ptrs.push(std::ptr::null());
    Some((owned, ptrs))
}

/// `MinecraftUtils::getLibCSymbols` — merge Rust libc-shim symbols into a map.
pub unsafe fn get_libc_symbols() -> HashMap<String, *mut c_void> {
    let mut out = HashMap::new();
    let len = get_shimmed_symbols_len();
    if len == 0 {
        return out;
    }
    let mut buf: Vec<ShimmedSymbol> = (0..len)
        .map(|_| ShimmedSymbol { name: std::ptr::null(), value: std::ptr::null_mut() })
        .collect();
    get_shimmed_symbols_fill(buf.as_mut_ptr());
    for s in &buf {
        if s.value.is_null() || s.name.is_null() {
            continue;
        }
        if let Ok(n) = CStr::from_ptr(s.name).to_str() {
            out.insert(n.to_string(), s.value);
        }
    }
    out
}

/// `MinecraftUtils::loadLibM` (`minecraft_utils.cpp:101`). Returns the OS handle.
pub unsafe fn load_lib_m() -> *mut c_void {
    let names = names_ptrs(LIB_M_SYMS);
    let val = match names {
        Some((owned, ptrs)) => crate::hybris_utils::mc_hybris_load_library_os(
            c"libm.so".as_ptr(),
            c"libm.so.6".as_ptr(),
            ptrs.as_ptr() as *mut *const c_char,
            std::ptr::null(),
            0,
        ),
        None => std::ptr::null_mut(),
    };
    if val.is_null() {
        log::error!("MinecraftUtils: Failed to load libm");
    }
    val
}

/// `MinecraftUtils::setupHybris` (`minecraft_utils.cpp:148`): load libz, then
/// register the mod API.
pub unsafe fn setup_hybris() {
    if let Some((owned, ptrs)) = names_ptrs(LIB_Z_SYMS) {
        crate::hybris_utils::mc_hybris_load_library_os(
            c"libz.so".as_ptr(),
            c"libz.so.1".as_ptr(),
            ptrs.as_ptr() as *mut *const c_char,
            std::ptr::null(),
            0,
        );
    }
    setup_api();
}

/// Build the mod API `syms` table -- the getApi `std::unordered_map`.
pub fn get_api() -> HashMap<String, *mut c_void> {
    let mut syms: HashMap<String, *mut c_void> = HashMap::new();
    syms.insert("mcpelauncher_log".to_string(), mc_mod_log as *mut c_void);
    syms.insert("mcpelauncher_vlog".to_string(), mc_mod_vlog as *mut c_void);

    syms.insert(
        "mcpelauncher_preinithook2".to_string(),
        mc_mod_preinithook2 as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_preinithook".to_string(),
        mc_mod_preinithook as *mut c_void,
    );

    syms.insert("mcpelauncher_hook".to_string(), mc_mod_hook as *mut c_void);
    syms.insert("mcpelauncher_hook2".to_string(), mc_mod_hook2 as *mut c_void);
    syms.insert(
        "mcpelauncher_hook2_add_library".to_string(),
        mc_mod_hook2_add_library as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_hook2_remove_library".to_string(),
        mc_mod_hook2_remove_library as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_hook2_delete".to_string(),
        mc_mod_hook2_delete as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_hook2_apply".to_string(),
        mc_mod_hook2_apply as *mut c_void,
    );

    syms.insert("mcpelauncher_patch".to_string(), mc_mod_patch as *mut c_void);

    syms.insert("mcpelauncher_host_dlopen".to_string(), libc::dlopen as *mut c_void);
    syms.insert("mcpelauncher_host_dlsym".to_string(), libc::dlsym as *mut c_void);
    syms.insert("mcpelauncher_host_dlclose".to_string(), libc::dlclose as *mut c_void);

    syms.insert(
        "mcpelauncher_relocate".to_string(),
        mc_mod_relocate as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_relocate2".to_string(),
        mc_mod_relocate2 as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_load_library".to_string(),
        mc_mod_load_library as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_unload_library".to_string(),
        mcpelauncher_dispatch_unload_library as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_dlclose_unlocked".to_string(),
        mcpelauncher_dispatch_dlclose as *mut c_void,
    );

    // Package/version exposed by address (stable statics in minecraft_version.rs).
    syms.insert(
        "mcpelauncher_package_name".to_string(),
        crate::minecraft_version::package_cstr() as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_package_version_code".to_string(),
        crate::minecraft_version::code_addr() as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_package_version_major".to_string(),
        crate::minecraft_version::major_addr() as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_package_version_minor".to_string(),
        crate::minecraft_version::minor_addr() as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_package_version_patch".to_string(),
        crate::minecraft_version::patch_addr() as *mut c_void,
    );
    syms.insert(
        "mcpelauncher_package_version_revision".to_string(),
        crate::minecraft_version::revision_addr() as *mut c_void,
    );

    syms.insert(
        "mcpelauncher_request_google_credentials".to_string(),
        mc_mod_request_google_credentials as *mut c_void,
    );
    syms.insert(
        "jnivm_register_method".to_string(),
        mc_mod_jnivm_register_method as *mut c_void,
    );

    syms
}

/// `MinecraftUtils::setupApi` (`minecraft_utils.cpp:598`): register getApi with
/// the linker as `libmcpelauncher_mod.so`. Returns number of symbols.
pub fn setup_api() -> usize {
    let syms = get_api();
    let mut keys: Vec<CString> = Vec::with_capacity(syms.len());
    let mut key_ptrs: Vec<*const c_char> = Vec::with_capacity(syms.len());
    let mut val_ptrs: Vec<*mut c_void> = Vec::with_capacity(syms.len());
    for (k, v) in &syms {
        if let Ok(c) = CString::new(k.as_bytes()) {
            keys.push(c);
            key_ptrs.push(keys.last().unwrap().as_ptr());
            val_ptrs.push(*v);
        }
    }
    let name = CString::new("libmcpelauncher_mod.so").unwrap();
    unsafe {
        linker_load_library_rust(
            name.as_ptr(),
            key_ptrs.as_ptr(),
            val_ptrs.as_ptr(),
            key_ptrs.len(),
        );
    }
    key_ptrs.len()
}

/// `#[no_mangle]` twin of `MinecraftUtils::getLibCSymbols` == the C++
/// `mc_get_libc_symbols` bridge behaviour: fill a caller-safe parallel array.
#[no_mangle]
pub unsafe extern "C" fn core_minecraft_utils_get_libc_symbols(
    buf: *mut ShimmedSymbol,
    max_entries: i32,
) -> i32 {
    if buf.is_null() || max_entries <= 0 {
        return 0;
    }
    let syms = get_libc_symbols();
    let mut count = 0i32;
    for (name, val) in &syms {
        if count >= max_entries {
            break;
        }
        let c = match CString::new(name.as_bytes()) {
            Ok(c) => c,
            Err(_) => continue,
        };
        // Leak each name so its pointer stays valid for the caller (mirrors the
        // C++ `static std::vector<std::string> persistent` reuse across calls).
        let leaked: &'static CString = Box::leak(Box::new(c));
        let cell = buf.add(count as usize);
        (*cell).name = leaked.as_ptr();
        (*cell).value = *val;
        count += 1;
    }
    count
}

/// `#[no_mangle]` twin: register merged libc symbols as `libc.so` with the linker
/// (replaces capi.cpp `MinecraftUtils::getLibCSymbols()` + `rust_load_stub("libc.so", ...)`).
#[no_mangle]
pub unsafe extern "C" fn core_minecraft_utils_register_libc_stub() {
    let syms = get_libc_symbols();
    let mut keys: Vec<CString> = Vec::with_capacity(syms.len());
    let mut key_ptrs: Vec<*const c_char> = Vec::with_capacity(syms.len());
    let mut val_ptrs: Vec<*mut c_void> = Vec::with_capacity(syms.len());
    for (k, v) in &syms {
        if let Ok(c) = CString::new(k.as_bytes()) {
            keys.push(c);
            key_ptrs.push(keys.last().unwrap().as_ptr());
            val_ptrs.push(*v);
        }
    }
    let name = CString::new("libc.so").unwrap();
    linker_load_library_rust(
        name.as_ptr(),
        key_ptrs.as_ptr(),
        val_ptrs.as_ptr(),
        key_ptrs.len(),
    );
}

/// `#[no_mangle]` twin of `MinecraftUtils::loadLibM`.
#[no_mangle]
pub unsafe extern "C" fn core_minecraft_utils_load_lib_m() -> *mut c_void {
    load_lib_m()
}

/// `#[no_mangle]` twin of `MinecraftUtils::setupHybris` (loads libz + setupApi).
#[no_mangle]
pub unsafe extern "C" fn core_minecraft_utils_setup_hybris() {
    setup_hybris();
}

// ---------------------------------------------------------------------------
// getApi intrinsics (registered as `void*` in libmcpelauncher_mod.so).
// ---------------------------------------------------------------------------

/// `mcpelauncher_preinithook2`: `(name, sym, user, callback)`.
unsafe extern "C" fn mc_mod_preinithook2(
    name: *const c_char,
    sym: *mut c_void,
    user: *mut c_void,
    callback: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
) {
    if name.is_null() {
        return;
    }
    let n = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    let e = PreinitEntry { value: sym, user, callback };
    if let Ok(c) = CString::new(n.as_bytes()) {
        preinit().lock().unwrap().insert(c, e);
    }
}

unsafe extern "C" fn def_callback(user: *mut c_void, orig: *mut c_void) {
    let slot = user as *mut *mut c_void;
    if !slot.is_null() {
        unsafe { *slot = orig };
    }
}

/// `mcpelauncher_preinithook`: `(name, sym, orig)`.
unsafe extern "C" fn mc_mod_preinithook(
    name: *const c_char,
    sym: *mut c_void,
    orig: *mut *mut c_void,
) {
    if name.is_null() {
        return;
    }
    let n = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return,
    };
    let e = if orig.is_null() {
        PreinitEntry { value: sym, user: std::ptr::null_mut(), callback: None }
    } else {
        PreinitEntry {
            value: sym,
            user: orig as *mut c_void,
            callback: Some(def_callback),
        }
    };
    if let Ok(c) = CString::new(n.as_bytes()) {
        preinit().lock().unwrap().insert(c, e);
    }
}

/// `mcpelauncher_hook`: resolve `sym`'s library, translate ctor name, hook it.
unsafe extern "C" fn mc_mod_hook(
    sym: *mut c_void,
    hook: *mut c_void,
    orig: *mut *mut c_void,
) -> *mut c_void {
    let mut info = DlInfo::default();
    if mcpelauncher_dispatch_dladdr(sym, &mut info) == 0 {
        log::error!("Hook: Failed to resolve hook for symbol {:x}", sym as usize);
        return std::ptr::null_mut();
    }
    let fname = info.dli_fname;
    let handle = mcpelauncher_dispatch_dlopen(fname, 0);
    let sym_name = if info.dli_sname.is_null() {
        None
    } else {
        match CStr::from_ptr(info.dli_sname).to_str() {
            Ok(s) => Some(s),
            Err(_) => None,
        }
    };
    let ret = if let Some(sname) = sym_name {
        let chosen = crate::hook::translate_constructor_name(sname)
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| sname.to_string());
        let cn = match CString::new(chosen.as_bytes()) {
            Ok(c) => c,
            Err(_) => return std::ptr::null_mut(),
        };
        crate::hook_manager::hook_manager_create_hook(handle, cn.as_ptr(), hook, orig)
    } else {
        std::ptr::null_mut()
    };
    mcpelauncher_dispatch_dlclose(handle);
    crate::hook_manager::hook_manager_apply_hooks();
    ret
}

/// `mcpelauncher_hook2`: `(lib, sym, hook, orig)`.
unsafe extern "C" fn mc_mod_hook2(
    lib: *mut c_void,
    sym: *const c_char,
    hook: *mut c_void,
    orig: *mut *mut c_void,
) -> *mut c_void {
    crate::hook_manager::hook_manager_create_hook(lib, sym, hook, orig)
}

unsafe extern "C" fn mc_mod_hook2_add_library(lib: *mut c_void) {
    crate::hook_manager::hook_manager_add_library(lib);
}
unsafe extern "C" fn mc_mod_hook2_remove_library(lib: *mut c_void) {
    crate::hook_manager::hook_manager_remove_library(lib);
}
unsafe extern "C" fn mc_mod_hook2_delete(hook: *mut c_void) {
    crate::hook_manager::hook_manager_delete_hook(hook);
}
unsafe extern "C" fn mc_mod_hook2_apply() {
    crate::hook_manager::hook_manager_apply_hooks();
}

unsafe extern "C" fn mc_mod_patch(
    address: *mut c_void,
    data: *mut c_void,
    size: usize,
) -> *mut c_void {
    if address.is_null() || data.is_null() || size == 0 {
        return address;
    }
    std::ptr::copy_nonoverlapping(data as *const u8, address as *mut u8, size);
    address
}

unsafe extern "C" fn mc_mod_relocate(handle: *mut c_void, name: *const c_char, hook: *mut c_void) {
    let n = if name.is_null() { std::ptr::null() } else { name };
    let keys = [n];
    let vals = [hook];
    mcpelauncher_dispatch_relocate(handle, keys.as_ptr(), vals.as_ptr(), 1);
}

unsafe extern "C" fn mc_mod_relocate2(
    handle: *mut c_void,
    count: usize,
    entries: *const HookEntry,
) {
    if entries.is_null() {
        return;
    }
    for i in 0..count {
        let e = &*entries.add(i);
        mc_mod_relocate(handle, e.name, e.hook);
    }
}

unsafe extern "C" fn mc_mod_load_library(
    name: *const c_char,
    count: usize,
    entries: *const HookEntry,
) {
    if name.is_null() || entries.is_null() {
        return;
    }
    let mut keys: Vec<*const c_char> = Vec::with_capacity(count);
    let mut vals: Vec<*mut c_void> = Vec::with_capacity(count);
    for i in 0..count {
        let e = &*entries.add(i);
        keys.push(e.name);
        vals.push(e.hook);
    }
    linker_load_library_rust(name, keys.as_ptr(), vals.as_ptr(), count);
}

// ---------------------------------------------------------------------------
// preinitHooks state + the extern "C" helpers consumed by minecraft_load.rs.
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct PreinitEntry {
    value: *mut c_void,
    user: *mut c_void,
    callback: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
}

/// Keyed by `CString` so `.as_ptr()` stays valid across the `loadMinecraftLib`
/// preinit window (mirrors C++ `std::unordered_map<std::string, HookEntry>`).
/// The wrapper exists because `PreinitEntry` holds raw pointers (not `Send`);
/// access is single-threaded (same invariant as the C++ HookManager).
struct PreinitTable(Mutex<HashMap<CString, PreinitEntry>>);
unsafe impl Send for PreinitTable {}
unsafe impl Sync for PreinitTable {}

fn preinit() -> &'static Mutex<HashMap<CString, PreinitEntry>> {
    static PREINIT: OnceLock<PreinitTable> = OnceLock::new();
    &PREINIT.get_or_init(|| PreinitTable(Mutex::new(HashMap::new()))).0
}

/// `#[no_mangle]` twin of `mc_find_data_file` (`minecraft_utils.cpp:626`).
#[no_mangle]
pub unsafe extern "C" fn mc_find_data_file(path: *const c_char) -> *const c_char {
    path_helper_find_data_file(path)
}

/// `#[no_mangle]` twin of `mc_get_preinit_hooks` (`minecraft_utils.cpp:630`).
#[no_mangle]
pub unsafe extern "C" fn mc_get_preinit_hooks(
    names: *mut *const c_char,
    vals: *mut *mut c_void,
    max: usize,
) -> usize {
    if max == 0 {
        return 0;
    }
    let pre = preinit().lock().unwrap();
    let mut i = 0usize;
    for (name, e) in pre.iter() {
        if i >= max {
            break;
        }
        if !names.is_null() {
            *names.add(i) = name.as_ptr();
        }
        if !vals.is_null() {
            *vals.add(i) = e.value;
        }
        i += 1;
    }
    i
}

/// `#[no_mangle]` twin of `mc_finalize_load` (`minecraft_utils.cpp:641`).
#[no_mangle]
pub unsafe extern "C" fn mc_finalize_load(
    handle: *mut c_void,
    names: *const *const c_char,
    vals: *const *mut c_void,
    count: usize,
) {
    for i in 0..count {
        let name = *names.add(i);
        if name.is_null() {
            continue;
        }
        let addr = mcpelauncher_dispatch_dlsym(handle, name);
        let name_s = match CStr::from_ptr(name).to_str() {
            Ok(s) => s,
            Err(_) => continue,
        };
        log::trace!(
            "MinecraftUtils: Found hook: {} @ {:p} (stub={:p})",
            name_s,
            addr,
            *vals.add(i)
        );
        let entry = preinit()
            .lock()
            .unwrap()
            .iter()
            .find(|(k, _)| k.as_bytes() == name_s.as_bytes())
            .map(|(_, e)| *e);
        if let Some(e) = entry {
            if let Some(cb) = e.callback {
                log::trace!("MinecraftUtils: with value: {:p}", *vals.add(i));
                cb(e.user, *vals.add(i));
            }
        }
    }
    crate::hook_manager::hook_manager_add_library(handle);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_api_has_required_keys() {
        let api = get_api();
        for k in [
            "mcpelauncher_log",
            "mcpelauncher_vlog",
            "mcpelauncher_preinithook",
            "mcpelauncher_preinithook2",
            "mcpelauncher_hook",
            "mcpelauncher_hook2",
            "mcpelauncher_hook2_add_library",
            "mcpelauncher_hook2_apply",
            "mcpelauncher_patch",
            "mcpelauncher_host_dlopen",
            "mcpelauncher_host_dlsym",
            "mcpelauncher_host_dlclose",
            "mcpelauncher_relocate",
            "mcpelauncher_relocate2",
            "mcpelauncher_load_library",
            "mcpelauncher_unload_library",
            "mcpelauncher_dlclose_unlocked",
            "mcpelauncher_package_name",
            "mcpelauncher_package_version_code",
            "mcpelauncher_package_version_major",
            "mcpelauncher_package_version_minor",
            "mcpelauncher_package_version_patch",
            "mcpelauncher_package_version_revision",
            "mcpelauncher_request_google_credentials",
            "jnivm_register_method",
        ] {
            assert!(api.contains_key(k), "missing getApi key {}", k);
        }
        for v in api.values() {
            assert!(!v.is_null(), "getApi entry with null fn pointer");
        }
    }

    #[test]
    fn get_libc_symbols_empty_when_shim_empty() {
        // Test stub returns len 0.
        let syms = unsafe { get_libc_symbols() };
        assert!(syms.is_empty());
    }

    #[test]
    fn preinit_push_and_query_roundtrip() {
        unsafe {
            mc_mod_preinithook2(c"sym1".as_ptr(), 0x11 as *mut c_void, 0x22 as *mut c_void, None);
            let mut names: [*const c_char; 4] = [std::ptr::null(); 4];
            let mut vals: [*mut c_void; 4] = [std::ptr::null_mut(); 4];
            let n = mc_get_preinit_hooks(names.as_mut_ptr(), vals.as_mut_ptr(), 4);
            assert_eq!(n, 1);
            if CStr::from_ptr(names[0]).to_str().unwrap() == "sym1" {
                assert_eq!(vals[0] as usize, 0x11);
            }
        }
    }
}

// Linker/FFI stubs so `cargo test -p corelib --lib` links without the client
// binary or the C++ shim. Tests only exercise true bodies via the twins above;
// these satisfy name resolution for code that is compiled but not executed.
#[cfg(test)]
mod test_stubs {
    use std::ffi::{c_char, c_void};
    use super::{DlInfo, ShimmedSymbol};

    #[no_mangle]
    pub unsafe extern "C" fn get_shimmed_symbols_len() -> usize { 0 }
    #[no_mangle]
    pub unsafe extern "C" fn get_shimmed_symbols_fill(_b: *mut ShimmedSymbol) {}
    #[no_mangle]
    pub unsafe extern "C" fn linker_load_library_rust(
        _n: *const c_char, _k: *const *const c_char, _v: *const *mut c_void, _l: usize,
    ) -> usize { 0 }
    #[no_mangle]
    pub unsafe extern "C" fn mcpelauncher_dispatch_dlsym(_h: *mut c_void, _s: *const c_char) -> *mut c_void { std::ptr::null_mut() }
    #[no_mangle]
    pub unsafe extern "C" fn mcpelauncher_dispatch_dladdr(_a: *const c_void, _i: *mut DlInfo) -> i32 { 0 }
    #[no_mangle]
    pub unsafe extern "C" fn mcpelauncher_dispatch_relocate(_h: *mut c_void, _k: *const *const c_char, _v: *const *mut c_void, _l: usize) {}
    #[no_mangle]
    pub unsafe extern "C" fn mcpelauncher_dispatch_unload_library(_h: *mut c_void) -> i32 { -1 }
    #[no_mangle]
    pub unsafe extern "C" fn path_helper_find_data_file(_p: *const c_char) -> *const c_char { std::ptr::null() }
    #[no_mangle]
    pub unsafe extern "C" fn mc_mod_log(_l: i32, _t: *const c_char, _f: *const c_char) {}
    #[no_mangle]
    pub unsafe extern "C" fn mc_mod_vlog(_l: i32, _t: *const c_char, _f: *const c_char) {}
    #[no_mangle]
    pub unsafe extern "C" fn mc_mod_request_google_credentials(_a: *const c_void, _b: *const c_void) {}
    #[no_mangle]
    pub unsafe extern "C" fn mc_mod_jnivm_register_method(
        _e: *mut c_void, _c: *mut c_void, _t: i32, _n: *const c_char, _s: *const c_char, _k: *mut c_void,
    ) -> bool { false }
}
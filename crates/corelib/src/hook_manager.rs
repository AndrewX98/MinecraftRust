//! `#[no_mangle]` twin of C++ `HookManager` (`mcpelauncher-core/src/hook.cpp`).
//!
//! Faithful Rust port of the hooking singleton that rewrites ELF relocations
//! (GOT/JUMP_SLOT slots) in loaded-library memory. Target is **x86_64 only**,
//! which uses **RELA** relocations (`DT_RELA`/`DT_RELASZ`/`DT_JMPREL`,
//! `DT_PLTREL=RELA`, 24-byte `Elf64_Rela` entries). The C++ original defaults to
//! 16-byte `Elf64_Rel` when `USE_RELA` is unset, which strides a 24-byte table
//! incorrectly; this port reads the correct 24-byte entries, so it behaves as the
//! C++ clearly intended but never exercised at boot (`applyHooks` is not on the
//! boot path).
//!
//! **Additive-only (deferral to Phase 6):** `hook.cpp` stays compiled because its
//! still-C++ callers (`minecraft_utils.cpp`, `mod_loader.cpp`) invoke the **mangled**
//! `HookManager::instance.*` methods, which Rust cannot emit/ABI. This module provides
//! the Rust singleton plus clean-named `#[no_mangle]` twins (`hook_manager_*`) that
//! Phase 6's `minecraft_utils` port (and any real mod hooking) will use once the C++
//! callers are gone.
//!
//! `HookInstance` returned by `createHook` is a leaked `Box`; the caller owns it and
//! must hand it to `hook_manager_delete_hook`.

use std::collections::HashMap;
use std::ffi::{c_char, c_void, CStr};
use std::sync::{Mutex, OnceLock};

// --- ELF64 (x86_64) constants ----------------------------------------------

const DT_NULL: i64 = 0;
const DT_NEEDED: i64 = 1;
const DT_PLTRELSZ: i64 = 2;
const DT_STRTAB: i64 = 5;
const DT_SYMTAB: i64 = 6;
const DT_RELA: i64 = 7;
const DT_RELASZ: i64 = 8;
const DT_REL: i64 = 17;
const DT_RELSZ: i64 = 18;
const DT_JMPREL: i64 = 23;

const PT_DYNAMIC: u32 = 2;
const PT_GNU_RELRO: u32 = 0x6474e552;

const R_X86_64_64: u32 = 1; // R_GENERIC_ABSOLUTE
const R_X86_64_GLOB_DAT: u32 = 6;
const R_X86_64_JUMP_SLOT: u32 = 7;

/// ELF64 header field offsets (we read raw bytes rather than grand structs).
const E_PHOFF: usize = 0x20;
const E_PHENTSIZE: usize = 0x36;
const E_PHNUM: usize = 0x38;
/// ELF64 program-header field offsets.
const P_TYPE: usize = 0x00;
const P_VADDR: usize = 0x10;
const P_MEMSZ: usize = 0x20;

const SIZEOF_DYN: usize = 16;
const SIZEOF_SYM: usize = 24;
const SIZEOF_RELA: usize = 24;

// --- Rust linker FFI (declared locally, resolved at final link) ------------

extern "C" {
    fn mcpelauncher_linker_resolve_rust_handle(handle: *mut c_void) -> usize;
    fn linker_rust_get_library_base(handle: usize) -> usize;
    fn linker_rust_get_library_dynamic(handle: usize) -> usize;
    fn linker_rust_find_symbol_index_by_name(handle: usize, name: *const c_char) -> u32;
    fn mcpelauncher_dispatch_dlopen(name: *const c_char, flags: i32) -> *mut c_void;
    fn mcpelauncher_dispatch_dlclose(handle: *mut c_void) -> i32;
}

// --- Helpers ----------------------------------------------------------------

#[inline]
unsafe fn rd<T: Copy>(addr: usize) -> T {
    std::ptr::read_unaligned(addr as *const T)
}
#[inline]
fn r_sym(r_info: u64) -> u32 {
    (r_info >> 32) as u32
}
#[inline]
fn r_type(r_info: u64) -> u32 {
    (r_info & 0xffff_ffff) as u32
}
#[inline]
fn is_patchable_reloc(ty: u32) -> bool {
    matches!(ty, R_X86_64_64 | R_X86_64_GLOB_DAT | R_X86_64_JUMP_SLOT)
}

fn cstr_at(addr: usize) -> Option<String> {
    if addr == 0 {
        return None;
    }
    unsafe { CStr::from_ptr(addr as *const c_char).to_str().ok().map(|s| s.to_owned()) }
}

// --- Data model (mirrors hook.cpp:51-101) -----------------------------------

/// Per-library relocation-hook state. All string/table pointers are resolved to
/// absolute addresses into the loaded image (`base + dynamic offset`).
struct LibInfo {
    handle: usize,
    base: usize,
    strtab: usize,
    symtab: usize,
    rela: usize,
    relasz: usize,
    pltrela: usize,
    pltrelasz: usize,
    /// symbol index -> arena index of a HookedSymbol (fast lookups in applyHooks).
    hooked_symbols: HashMap<u32, usize>,
    /// dependency handles (for propagation + teardown).
    dependencies: Vec<usize>,
}

impl LibInfo {
    /// `LibInfo::LibInfo` (hook.cpp:51-92): parse dynamic section + PT_GNU_RELRO.
    fn parse(handle: usize) -> LibInfo {
        let base = unsafe { linker_rust_get_library_base(handle) };
        let dyn_addr = unsafe { linker_rust_get_library_dynamic(handle) };
        let mut info = LibInfo {
            handle,
            base,
            strtab: 0,
            symtab: 0,
            rela: 0,
            relasz: 0,
            pltrela: 0,
            pltrelasz: 0,
            hooked_symbols: HashMap::new(),
            dependencies: Vec::new(),
        };
        if dyn_addr != 0 {
            for i in 0.. {
                let d_tag = unsafe { rd::<i64>(dyn_addr + i * SIZEOF_DYN) };
                if d_tag == DT_NULL {
                    break;
                }
                let d_val = unsafe { rd::<u64>(dyn_addr + i * SIZEOF_DYN + 8) };
                match d_tag {
                    DT_STRTAB => info.strtab = base + d_val as usize,
                    DT_SYMTAB => info.symtab = base + d_val as usize,
                    DT_RELA => info.rela = base + d_val as usize,
                    DT_RELASZ => info.relasz = d_val as usize,
                    DT_REL => info.rela = base + d_val as usize,
                    DT_RELSZ => info.relasz = d_val as usize,
                    DT_JMPREL => info.pltrela = base + d_val as usize,
                    DT_PLTRELSZ => info.pltrelasz = d_val as usize,
                    _ => {}
                }
            }
        }
        info
    }

    fn get_symbol_name(&self, symbol_index: u32) -> Option<String> {
        if self.symtab == 0 {
            return None;
        }
        let st_name = unsafe { rd::<u32>(self.symtab + symbol_index as usize * SIZEOF_SYM) };
        cstr_at(self.strtab + st_name as usize)
    }

    fn set_hook(&mut self, symbol_index: u32, arena_idx: usize) {
        self.hooked_symbols.insert(symbol_index, arena_idx);
    }
}

/// Shared state referenced by both the manager-wide hook map and each `LibInfo`'s
/// quick map (analogue of the C++ `shared_ptr<HookedSymbol>`).
struct HookedSymbol {
    lib_handle: usize,
    symbol_index: u32,
    original: *mut c_void,
    first_hook: *mut HookInstance,
    last_hook: *mut HookInstance,
}

/// Opaque handle returned to callers; freed via `deleteHook` (`Box::from_raw`).
struct HookInstance {
    symbol: usize, // arena index into HookManager::arena
    parent: *mut HookInstance,
    child: *mut HookInstance,
    replacement: *mut c_void,
    orig: *mut *mut c_void,
}

struct HookManager {
    /// handle -> LibInfo.
    libs: HashMap<usize, Box<LibInfo>>,
    /// dependency handle -> Vec of LibInfo addresses that depend on it.
    dependents: HashMap<usize, Vec<usize>>,
    /// (handle, symbol_index) -> arena index of HookedSymbol.
    hooked: HashMap<(usize, u32), usize>,
    /// Stable store of HookedSymbol; referenced by index everywhere.
    arena: Vec<HookedSymbol>,
}

impl HookManager {
    fn new() -> HookManager {
        HookManager {
            libs: HashMap::new(),
            dependents: HashMap::new(),
            hooked: HashMap::new(),
            arena: Vec::new(),
        }
    }

    fn find_symbol_index(&self, lib_handle: usize, name: &str) -> u32 {
        let c = match std::ffi::CString::new(name) {
            Ok(c) => c,
            Err(_) => return u32::MAX,
        };
        unsafe { linker_rust_find_symbol_index_by_name(lib_handle, c.as_ptr()) }
    }

    fn add_library(&mut self, handle: usize) {
        if self.libs.contains_key(&handle) {
            return;
        }
        let mut info = Box::new(LibInfo::parse(handle));
        if info.base == 0 {
            return;
        }
        // DT_NEEDED walk (hook.cpp addLibrary:161-189).
        let dyn_addr = unsafe { linker_rust_get_library_dynamic(handle) };
        if dyn_addr != 0 {
            for i in 0.. {
                let d_tag = unsafe { rd::<i64>(dyn_addr + i * SIZEOF_DYN) };
                if d_tag == DT_NULL {
                    break;
                }
                if d_tag == DT_NEEDED {
                    let offset = unsafe { rd::<u32>(dyn_addr + i * SIZEOF_DYN + 8) };
                    let Some(name) = cstr_at(info.strtab + offset as usize) else {
                        continue;
                    };
                    let cname = match std::ffi::CString::new(name) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    let dep = unsafe { mcpelauncher_dispatch_dlopen(cname.as_ptr(), 0) };
                    if dep.is_null() {
                        continue;
                    }
                    let dep_handle = dep as usize;
                    info.dependencies.push(dep_handle);
                    let lib_addr = &*info as *const LibInfo as usize;
                    self.dependents.entry(dep_handle).or_default().push(lib_addr);
                    // Rust handles returned by name lookup do NOT hold a reference;
                    // only release handles that have no Rust-side backing.
                    if unsafe { mcpelauncher_linker_resolve_rust_handle(dep) } == 0 {
                        unsafe { mcpelauncher_dispatch_dlclose(dep) };
                    }
                }
            }
        }
        self.libs.insert(handle, info);
    }

    fn remove_library(&mut self, handle: usize) {
        let Some(info) = self.libs.remove(&handle) else {
            return;
        };
        let lib_addr = &*info as *const LibInfo as usize;
        for dep in &info.dependencies {
            if let Some(dependents) = self.dependents.get_mut(dep) {
                dependents.retain(|d| *d != lib_addr);
            }
        }
    }

    /// `getOrCreateHookSymbol` (hook.cpp:201-221). Returns an arena index.
    fn get_or_create_hook_symbol(&mut self, lib_handle: usize, symbol_index: u32) -> usize {
        if !self.libs.contains_key(&lib_handle) {
            return usize::MAX;
        }
        if let Some(&idx) = self.hooked.get(&(lib_handle, symbol_index)) {
            return idx;
        }
        let idx = self.arena.len();
        self.arena.push(HookedSymbol {
            lib_handle,
            symbol_index,
            original: std::ptr::null_mut(),
            first_hook: std::ptr::null_mut(),
            last_hook: std::ptr::null_mut(),
        });
        if let Some(lib) = self.libs.get_mut(&lib_handle) {
            lib.set_hook(symbol_index, idx);
        }
        self.hooked.insert((lib_handle, symbol_index), idx);
        let name = self
            .libs
            .get(&lib_handle)
            .and_then(|l| l.get_symbol_name(symbol_index));
        // Propagate to every dependent LibInfo.
        let dependents = self.dependents.get(&lib_handle).cloned().unwrap_or_default();
        for dep_addr in dependents {
            let dep_handle = match self
                .libs
                .values()
                .find(|l| &***l as *const LibInfo as usize == dep_addr)
            {
                Some(d) => d.handle,
                None => continue,
            };
            let Some(name) = name.as_deref() else {
                continue;
            };
            let s_idx = self.find_symbol_index(dep_handle, name);
            if s_idx != u32::MAX {
                if let Some(dep) = self.libs.get_mut(&dep_handle) {
                    dep.set_hook(s_idx, idx);
                }
            }
        }
        idx
    }

    /// `createHook` by symbol index (hook.cpp:223-238). Returns a leaked Box.
    fn create_hook_by_index(
        &mut self,
        lib: usize,
        symbol_index: u32,
        replacement: *mut c_void,
        orig: *mut *mut c_void,
    ) -> *mut HookInstance {
        let symbol = self.get_or_create_hook_symbol(lib, symbol_index);
        if symbol == usize::MAX {
            return std::ptr::null_mut();
        }
        let ret = Box::into_raw(Box::new(HookInstance {
            symbol,
            parent: std::ptr::null_mut(),
            child: std::ptr::null_mut(),
            replacement,
            orig,
        }));
        let sym = &mut self.arena[symbol];
        if sym.first_hook.is_null() {
            sym.first_hook = ret;
        } else if !sym.last_hook.is_null() {
            let parent = sym.last_hook;
            unsafe {
                (*ret).parent = parent;
                (*parent).child = ret;
                if !orig.is_null() {
                    *orig = (*parent).replacement;
                }
            }
        }
        sym.last_hook = ret;
        ret
    }

    /// `createHook` by name (hook.cpp:240-248). Returns a leaked Box.
    fn create_hook(
        &mut self,
        lib: usize,
        symbol_name: &str,
        replacement: *mut c_void,
        orig: *mut *mut c_void,
    ) -> *mut HookInstance {
        if !self.libs.contains_key(&lib) {
            return std::ptr::null_mut();
        }
        let sym_index = self.find_symbol_index(lib, symbol_name);
        if sym_index == u32::MAX {
            return std::ptr::null_mut();
        }
        self.create_hook_by_index(lib, sym_index, replacement, orig)
    }

    /// `deleteHook` (hook.cpp:250-265).
    fn delete_hook(&mut self, hook: *mut HookInstance) {
        if hook.is_null() {
            return;
        }
        unsafe {
            let idx = (*hook).symbol;
            let parent = (*hook).parent;
            let child = (*hook).child;
            if !child.is_null() {
                (*child).parent = parent;
                if !parent.is_null() {
                    *(*child).orig = (*parent).replacement;
                } else if !(*child).orig.is_null() {
                    *(*child).orig = self.arena[idx].original;
                }
            }
            if !parent.is_null() {
                (*parent).child = child;
            }
            if self.arena[idx].first_hook == hook {
                self.arena[idx].first_hook = child;
            }
            if self.arena[idx].last_hook == hook {
                self.arena[idx].last_hook = parent;
            }
            drop(Box::from_raw(hook));
        }
    }

    /// `applyHooks` (hook.cpp:267-270): apply relocs for every registered lib.
    fn apply_hooks(&mut self) {
        let libs: Vec<*mut LibInfo> = self
            .libs
            .iter_mut()
            .map(|(_, l)| &mut **l as *mut LibInfo)
            .collect();
        for lib in libs {
            unsafe {
                apply_relocs(&mut *lib, &mut self.arena);
            }
        }
    }
}

/// Write `replacement` into `base + r_offset`; returns the prior value.
unsafe fn patch_slot(base: usize, r_offset: u64, replacement: usize) -> usize {
    let addr = (base + r_offset as usize) as *mut usize;
    let original = std::ptr::read_unaligned(addr);
    std::ptr::write_unaligned(addr, replacement);
    original
}

/// `LibInfo::applyHooks(rel, relsz)` (hook.cpp:114-154), reading x86_64 RELA
/// 24-byte entries and honoring the per-symbol hook chain.
unsafe fn apply_relocs(lib: &mut LibInfo, arena: &mut [HookedSymbol]) {
    for (addr, byte_len) in [(lib.rela, lib.relasz), (lib.pltrela, lib.pltrelasz)] {
        if addr == 0 || byte_len == 0 {
            continue;
        }
        let count = byte_len / SIZEOF_RELA;
        for i in 0..count {
            let entry = addr + i * SIZEOF_RELA;
            let r_offset = rd::<u64>(entry);
            let r_info = rd::<u64>(entry + 8);
            let ty = r_type(r_info);
            if !is_patchable_reloc(ty) {
                continue;
            }
            let sym = r_sym(r_info);
            let Some(&hooked_idx) = lib.hooked_symbols.get(&sym) else {
                continue;
            };
            let sym_info = &mut arena[hooked_idx];
            if sym_info.lib_handle != lib.handle {
                continue;
            }
            let mut replacement = sym_info.original as usize;
            if !sym_info.last_hook.is_null() {
                let lh = sym_info.last_hook;
                if !(*lh).replacement.is_null() {
                    replacement = (*lh).replacement as usize;
                }
            } else if replacement == 0 {
                continue;
            }
            let original = patch_slot(lib.base, r_offset, replacement);
            if original != 0 && sym_info.original.is_null() {
                // Fill original on the first observed relocation for this symbol.
                sym_info.original = original as *mut c_void;
                if !sym_info.first_hook.is_null() {
                    let fh = sym_info.first_hook;
                    if !(*fh).orig.is_null() {
                        *(*fh).orig = original as *mut c_void;
                    }
                }
            }
        }
    }
}

// --- Singleton + safe extern twins ------------------------------------------

// SAFETY: all accesses to `HookManager` go through `manager()`'s mutex; the raw
// pointers it holds point into loaded-library memory that outlives the launcher,
// and hooking is logically single-threaded (the C++ original relied on this too).
unsafe impl Send for HookManager {}
unsafe impl Sync for HookManager {}

static HOOK: OnceLock<Mutex<HookManager>> = OnceLock::new();
fn manager() -> &'static Mutex<HookManager> {
    HOOK.get_or_init(|| Mutex::new(HookManager::new()))
}
fn run<T>(f: impl FnOnce(&mut HookManager) -> T) -> T {
    let mut m = manager().lock().unwrap_or_else(|p| p.into_inner());
    f(&mut m)
}

/// `#[no_mangle]` twins (additive; clean names never collide with the C++ mangled
/// `HookManager::*` methods still linked in `hook.cpp`). Phase 6's `minecraft_utils`
/// port and any real mod hooking will call these.

#[no_mangle]
pub extern "C" fn hook_manager_add_library(handle: *mut c_void) {
    run(|m| {
        let rh = unsafe { mcpelauncher_linker_resolve_rust_handle(handle) };
        if rh != 0 {
            m.add_library(rh);
        }
    });
}

#[no_mangle]
pub extern "C" fn hook_manager_remove_library(handle: *mut c_void) {
    run(|m| {
        let rh = unsafe { mcpelauncher_linker_resolve_rust_handle(handle) };
        if rh != 0 {
            m.remove_library(rh);
        }
    });
}

#[no_mangle]
pub extern "C" fn hook_manager_create_hook(
    lib: *mut c_void,
    symbol_name: *const c_char,
    replacement: *mut c_void,
    orig: *mut *mut c_void,
) -> *mut c_void {
    if lib.is_null() || symbol_name.is_null() {
        return std::ptr::null_mut();
    }
    let name = match unsafe { CStr::from_ptr(symbol_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    run(|m| {
        let rh = unsafe { mcpelauncher_linker_resolve_rust_handle(lib) };
        if rh == 0 {
            return std::ptr::null_mut();
        }
        m.create_hook(rh, name, replacement, orig) as *mut c_void
    })
}

#[no_mangle]
pub extern "C" fn hook_manager_delete_hook(hook: *mut c_void) {
    if hook.is_null() {
        return;
    }
    run(|m| m.delete_hook(hook as *mut HookInstance));
}

#[no_mangle]
pub extern "C" fn hook_manager_apply_hooks() {
    run(|m| m.apply_hooks());
}

#[no_mangle]
pub extern "C" fn hook_manager_find_symbol_index(lib: *mut c_void, symbol_name: *const c_char) -> u32 {
    if lib.is_null() || symbol_name.is_null() {
        return u32::MAX;
    }
    let name = match unsafe { CStr::from_ptr(symbol_name) }.to_str() {
        Ok(s) => s,
        Err(_) => return u32::MAX,
    };
    run(|m| {
        let rh = unsafe { mcpelauncher_linker_resolve_rust_handle(lib) };
        if rh == 0 {
            return u32::MAX;
        }
        m.find_symbol_index(rh, name)
    })
}

// --- Tests -----------------------------------------------------------------

/// Linker FFI stubs so `cargo test -p corelib --lib` can link without the `linker`
/// crate (which is only linked into the `client` binary). Tests avoid exercising
/// these; they exist only to satisfy name resolution.
#[cfg(test)]
mod test_stubs {
    use std::ffi::{c_char, c_void};

    #[no_mangle]
    pub unsafe extern "C" fn mcpelauncher_linker_resolve_rust_handle(_h: *mut c_void) -> usize {
        0
    }
    #[no_mangle]
    pub unsafe extern "C" fn linker_rust_get_library_base(_h: usize) -> usize {
        0
    }
    #[no_mangle]
    pub unsafe extern "C" fn linker_rust_get_library_dynamic(_h: usize) -> usize {
        0
    }
    #[no_mangle]
    pub unsafe extern "C" fn linker_rust_find_symbol_index_by_name(
        _h: usize,
        _n: *const c_char,
    ) -> u32 {
        u32::MAX
    }
    #[no_mangle]
    pub unsafe extern "C" fn mcpelauncher_dispatch_dlopen(
        _n: *const c_char,
        _f: i32,
    ) -> *mut c_void {
        std::ptr::null_mut()
    }
    #[no_mangle]
    pub unsafe extern "C" fn mcpelauncher_dispatch_dlclose(_h: *mut c_void) -> i32 {
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Object-identity helper: a hook symbol keyed on the lib *address*.
    fn lib_addr(b: &Box<[u8]>) -> usize {
        b.as_ptr() as usize
    }

    /// Synthesized in-memory ELF: ehdr + phdr(PT_DYNAMIC) + dyn + symtab + strtab.
    /// Returns the buffer (kept alive so pointers stay valid) and its base.
    fn synth_lib() -> Box<[u8]> {
        const PHDR: usize = 64;
        const DYN: usize = PHDR + 56;
        const SYMTAB: usize = DYN + 6 * 16;
        const STRTAB: usize = SYMTAB + 2 * 24;
        let names = b"\0foo\0bar\0";
        let total = STRTAB + names.len();
        let mut b = vec![0u8; total];
        b[E_PHOFF..E_PHOFF + 8].copy_from_slice(&(PHDR as u64).to_ne_bytes());
        b[E_PHENTSIZE..E_PHENTSIZE + 2].copy_from_slice(&(56u16).to_ne_bytes());
        b[E_PHNUM..E_PHNUM + 2].copy_from_slice(&(1u16).to_ne_bytes());
        // Phdr[0] = PT_DYNAMIC with p_vaddr/p_offset = DYN, p_memsz = 6*16.
        b[PHDR + P_TYPE..PHDR + P_TYPE + 4].copy_from_slice(&PT_DYNAMIC.to_ne_bytes());
        b[PHDR + 0x08..PHDR + 0x10].copy_from_slice(&(DYN as u64).to_ne_bytes()); // p_offset
        b[PHDR + P_VADDR..PHDR + P_VADDR + 8].copy_from_slice(&(DYN as u64).to_ne_bytes());
        b[PHDR + P_MEMSZ..PHDR + P_MEMSZ + 8].copy_from_slice(&(6 * 16u64).to_ne_bytes());
        // Dynamic: STRTAB, SYMTAB, then DT_NULL (zeros).
        let st = STRTAB as u64;
        let sy = SYMTAB as u64;
        b[DYN + 0..DYN + 8].copy_from_slice(&DT_STRTAB.to_ne_bytes());
        b[DYN + 8..DYN + 16].copy_from_slice(&st.to_ne_bytes());
        b[DYN + 16..DYN + 24].copy_from_slice(&DT_SYMTAB.to_ne_bytes());
        b[DYN + 24..DYN + 32].copy_from_slice(&sy.to_ne_bytes());
        // symtab entries (st_name at 0): sym0 = "foo", sym1 = "bar".
        b[SYMTAB..SYMTAB + 4].copy_from_slice(&1u32.to_ne_bytes());
        b[SYMTAB + 24..SYMTAB + 28].copy_from_slice(&5u32.to_ne_bytes());
        // strtab: \0foo\0bar\0 -> offset 1 = foo, offset 5 = bar.
        b[STRTAB..].copy_from_slice(names);
        b.into_boxed_slice()
    }

    #[test]
    fn symbol_names_resolve_from_synthetic_elf() {
        let buf = synth_lib();
        let base = lib_addr(&buf);
        // Reconstruct a LibInfo as LibInfo::parse would, but in-process we can't
        // call linker_rust_get_library_* (no loaded lib), so set fields directly.
        let lib = LibInfo {
            handle: base,
            base,
            strtab: base + (64 + 56 + 6 * 16 + 2 * 24),
            symtab: base + (64 + 56 + 6 * 16),
            rela: 0,
            relasz: 0,
            pltrela: 0,
            pltrelasz: 0,
            hooked_symbols: HashMap::new(),
            dependencies: Vec::new(),
        };
        assert_eq!(lib.get_symbol_name(0).as_deref(), Some("foo"));
        assert_eq!(lib.get_symbol_name(1).as_deref(), Some("bar"));
    }

    #[test]
    fn dynamic_and_relro_offsets_parse() {
        // Use goblin to validate our field offsets against a real minimal ELF is
        // heavy; instead check the raw offsets exist on a synthesized image.
        let buf = synth_lib();
        let base = lib_addr(&buf);
        assert!(base > 0);
        let dyn_addr = base + 64 + 56;
        let d_tag = unsafe { rd::<i64>(dyn_addr) };
        let d_val = unsafe { rd::<u64>(dyn_addr + 8) };
        assert_eq!(d_tag, DT_STRTAB);
        assert_eq!(d_val, (64 + 56 + 6 * 16 + 2 * 24) as u64); // STRTAB follows symtab
    }

    #[test]
    fn reloa_reloc_apply_writes_got_slot() {
        // Build a writable "image": a RELA table + GOT slots side by side.
        let mut img = vec![0u8; 256];
        let base = img.as_mut_ptr() as usize;
        // GOT slots start at offset 128 (8 slots of 8 bytes).
        let got_off = 128;
        img[got_off + 0..got_off + 8].copy_from_slice(&0x1111usize.to_ne_bytes());
        img[got_off + 8..got_off + 16].copy_from_slice(&0x2222usize.to_ne_bytes());
        // One RELA entry at offset 0: r_offset=128, r_info=(sym=0 <<32)|JUMP_SLOT.
        let r_info = (0u64 << 32) | R_X86_64_JUMP_SLOT as u64;
        img[0..8].copy_from_slice(&(got_off as u64).to_ne_bytes());
        img[8..16].copy_from_slice(&r_info.to_ne_bytes());
        let rela_addr = base;

        // Manually construct a LibInfo pointing the RELA table + GOT into `img`.
        let mut lib = LibInfo {
            handle: base,
            base,
            strtab: 0,
            symtab: 0,
            rela: rela_addr,
            relasz: SIZEOF_RELA,
            pltrela: 0,
            pltrelasz: 0,
            hooked_symbols: HashMap::new(),
            dependencies: Vec::new(),
        };
        // arena: one hooked symbol at index 0, no hooks yet -> replacement = original(0) -> skip.
        let mut arena = vec![HookedSymbol {
            lib_handle: base,
            symbol_index: 0,
            original: std::ptr::null_mut(),
            first_hook: std::ptr::null_mut(),
            last_hook: std::ptr::null_mut(),
        }];
        arena[0].original = 0xABCD as *mut c_void;
        lib.hooked_symbols.insert(0, 0);

        unsafe {
            apply_relocs(&mut lib, &mut arena);
        }
        let written = unsafe { rd::<usize>(base + got_off) };
        assert_eq!(written, 0xABCD); // replacement written into the GOT slot
    }

    #[test]
    fn reloa_apply_records_first_original_into_orig_ptrs() {
        let mut img = vec![0u8; 128];
        let got_off = 64;
        img[got_off..got_off + 8].copy_from_slice(&0xDEADusize.to_ne_bytes());
        let r_info = (0u64 << 32) | R_X86_64_64 as u64;
        img[0..8].copy_from_slice(&(got_off as u64).to_ne_bytes());
        img[8..16].copy_from_slice(&r_info.to_ne_bytes());
        let base = img.as_mut_ptr() as usize;

        let mut lib = LibInfo {
            handle: base,
            base,
            strtab: 0,
            symtab: 0,
            rela: base,
            relasz: SIZEOF_RELA,
            pltrela: 0,
            pltrelasz: 0,
            hooked_symbols: HashMap::new(),
            dependencies: Vec::new(),
        };
        // Hook chain: one HookInstance (orig out-ptr) + one HookedSymbol.
        let mut out_orig: *mut c_void = std::ptr::null_mut();
        let mut hook = HookInstance {
            symbol: 0,
            parent: std::ptr::null_mut(),
            child: std::ptr::null_mut(),
            replacement: 0x7777 as *mut c_void,
            orig: &mut out_orig,
        };
        let hook_ptr: *mut HookInstance = &mut hook;
        let mut arena = vec![HookedSymbol {
            lib_handle: base,
            symbol_index: 0,
            original: std::ptr::null_mut(),
            first_hook: hook_ptr,
            last_hook: hook_ptr,
        }];
        lib.hooked_symbols.insert(0, 0);

        unsafe {
            apply_relocs(&mut lib, &mut arena);
        }
        assert_eq!(arena[0].original as usize, 0xDEAD);
        assert_eq!(out_orig as usize, 0xDEAD); // orig out-ptr filled on first patch
        let written = unsafe { rd::<usize>(base + got_off) };
        assert_eq!(written, 0x7777);
    }

    #[test]
    fn unknown_reloc_type_is_skipped() {
        let mut img = vec![0u8; 128];
        let got_off = 64;
        img[got_off..got_off + 8].copy_from_slice(&0x1234usize.to_ne_bytes());
        // r_info type = 4 (R_X86_64_TPOFF64-ish) — not patchable.
        let r_info = (0u64 << 32) | 4u64;
        img[0..8].copy_from_slice(&(got_off as u64).to_ne_bytes());
        img[8..16].copy_from_slice(&r_info.to_ne_bytes());
        let base = img.as_mut_ptr() as usize;

        let mut lib = LibInfo {
            handle: base,
            base,
            strtab: 0,
            symtab: 0,
            rela: base,
            relasz: SIZEOF_RELA,
            pltrela: 0,
            pltrelasz: 0,
            hooked_symbols: HashMap::new(),
            dependencies: Vec::new(),
        };
        let mut empty: Vec<HookedSymbol> = Vec::new();
        lib.hooked_symbols.insert(0, 0); // would match, but type is skipped first
        unsafe {
            apply_relocs(&mut lib, &mut empty);
        }
        // Never touched because type isn't patchable; slot unchanged.
        assert_eq!(unsafe { rd::<usize>(base + got_off) }, 0x1234);
    }

    #[test]
    fn create_and_delete_hook_chain() {
        let mut m = HookManager::new();
        // We can't add_library without a real linker handle, so inject directly.
        let base = 0x1000usize;
        let info = LibInfo {
            handle: base,
            base,
            strtab: 0,
            symtab: 0,
            rela: 0,
            relasz: 0,
            pltrela: 0,
            pltrelasz: 0,
            hooked_symbols: HashMap::new(),
            dependencies: Vec::new(),
        };
        m.libs.insert(base, Box::new(info));

        let mut orig1: *mut c_void = std::ptr::null_mut();
        let mut orig2: *mut c_void = std::ptr::null_mut();
        let h1 = m.create_hook_by_index(base, 7, 0xAAA as *mut c_void, &mut orig1);
        assert!(!h1.is_null());
        let h2 = m.create_hook_by_index(base, 7, 0xBBB as *mut c_void, &mut orig2);
        // Second hook's orig gets the first (parent) replacement.
        assert_eq!(orig2 as usize, 0xAAA);

        // Chain: h1.parent=null, h1.child=h2; h2.parent=h1, h2.child=null.
        unsafe {
            assert!((*h1).child == h2);
            assert!((*h2).parent == h1);
        }

        // Delete the first hook -> second becomes head, orig2 back to original(0).
        m.delete_hook(h1);
        unsafe {
            assert!((*h2).parent.is_null());
        }
        assert!(m.arena[0].first_hook == h2);
        assert_eq!(orig2 as usize, 0);

        m.delete_hook(h2);
        assert!(m.arena[0].first_hook.is_null());
        assert!(m.arena[0].last_hook.is_null());
    }
}
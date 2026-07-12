use crate::soinfo::SoInfo;
use crate::tls;

// ============================================================================
// Relocation type constants (x86_64)
// ============================================================================

const R_GENERIC_NONE: u32 = 0;
const R_X86_64_64: u32 = 1;       // R_GENERIC_ABSOLUTE
const R_X86_64_PC32: u32 = 2;
const R_X86_64_COPY: u32 = 5;     // R_GENERIC_COPY
const R_X86_64_GLOB_DAT: u32 = 6; // R_GENERIC_GLOB_DAT
const R_X86_64_JUMP_SLOT: u32 = 7; // R_GENERIC_JUMP_SLOT
const R_X86_64_RELATIVE: u32 = 8; // R_GENERIC_RELATIVE
const R_X86_64_32: u32 = 10;
const R_X86_64_DTPMOD64: u32 = 17; // R_GENERIC_TLS_DTPMOD
const R_X86_64_DTPOFF64: u32 = 18; // R_GENERIC_TLS_DTPREL
const R_X86_64_TPOFF64: u32 = 19;  // R_GENERIC_TLS_TPREL
const R_X86_64_TLSDESC: u32 = 36;
const R_X86_64_IRELATIVE: u32 = 37;

fn r_type(r_info: u64) -> u32 {
    (r_info & 0xffffffff) as u32
}

fn r_sym(r_info: u64) -> u32 {
    (r_info >> 32) as u32
}

// ============================================================================
// Relocation statistics (optional, matches linker_stat_t)
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelocationKind {
    Absolute = 0,
    Relative,
    Symbol,
    SymbolCached,
}

static mut LINKER_STATS: [i32; 4] = [0; 4];

pub fn count_relocation(kind: RelocationKind) {
    unsafe {
        LINKER_STATS[kind as usize] += 1;
    }
}

pub fn print_linker_stats() {
    let counts = unsafe { LINKER_STATS };
    log::info!(
        "RELO STATS: {} abs, {} rel, {} sym ({} cached)",
        counts[0], counts[1], counts[2], counts[3],
    );
}

pub fn reset_linker_stats() {
    unsafe { LINKER_STATS = [0; 4] };
}

// ============================================================================
// Symbol cache (1-entry, matches C++ Relocator cache)
// ============================================================================

#[derive(Clone)]
struct SymCacheEntry {
    sym_val: u32,
    found_in_base: usize,
    sym_value: usize,
}

struct SymCache {
    entry: Option<SymCacheEntry>,
    hits: i32,
}

impl SymCache {
    fn new() -> Self {
        SymCache { entry: None, hits: 0 }
    }

    fn lookup(
        &mut self,
        r_sym: u32,
        sym_name: &str,
        si: &SoInfo,
        get_symbol: &dyn Fn(&str) -> Option<usize>,
    ) -> Option<(usize, usize)> {
        if let Some(ref cache) = self.entry {
            if cache.sym_val == r_sym {
                self.hits += 1;
                return Some((cache.found_in_base, cache.sym_value));
            }
        }

        let result = resolve_symbol(si, sym_name, get_symbol);
        if let Some((base, val)) = result {
            self.entry = Some(SymCacheEntry {
                sym_val: r_sym,
                found_in_base: base,
                sym_value: val,
            });
        } else {
            self.entry = None;
        }
        result
    }
}

fn resolve_symbol(
    si: &SoInfo,
    sym_name: &str,
    get_symbol: &dyn Fn(&str) -> Option<usize>,
) -> Option<(usize, usize)> {
    if sym_name.is_empty() {
        return None;
    }
    // Check external symbols first
    if let Some(&addr) = si.external_symbols.get(sym_name) {
        return Some((addr, addr));
    }
    // Fall through to the global resolver
    get_symbol(sym_name).map(|addr| (addr, addr))
}

fn sym_name_from_soinfo(si: &SoInfo, sym_idx: u32) -> String {
    if sym_idx == 0 {
        return String::new();
    }
    if let Some(symtab) = si.symtab {
        if let Some(strtab) = si.strtab {
            unsafe {
                let sym_ptr = (symtab as *const u8).add(sym_idx as usize * 24) as *const u32;
                let str_offset = sym_ptr.read() as usize;
                if str_offset < si.strtab_size {
                    let name_ptr = (strtab as *const u8).add(str_offset);
                    let cstr = std::ffi::CStr::from_ptr(name_ptr as *const i8);
                    return cstr.to_str().unwrap_or("").to_string();
                }
            }
        }
    }
    String::new()
}

// ============================================================================
// IFUNC resolver support
// ============================================================================

pub type IfuncResolver = unsafe extern "C" fn() -> *mut std::ffi::c_void;

/// Call an IFUNC resolver function. The resolver returns a function pointer
/// to the implementation.
unsafe fn call_ifunc_resolver(addr: usize) -> usize {
    let resolver: IfuncResolver = std::mem::transmute(addr);
    resolver() as usize
}

// ============================================================================
// Relocation entry types (repr(C) for direct memory access)
// ============================================================================

#[repr(C)]
#[derive(Clone, Copy)]
struct Rela {
    r_offset: u64,
    r_info: u64,
    r_addend: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Rel {
    r_offset: u64,
    r_info: u64,
}

// ============================================================================
// Individual relocation processing
// ============================================================================

#[derive(Debug)]
pub enum RelocError {
    UnsupportedType { r_type: u32, r_sym: u32 },
    SymbolNotFound { sym_name: String },
    TlsError(String),
    CopyRelocNotSupported,
    TextRelocFailed(String),
    BadRelocation(String),
}

impl std::fmt::Display for RelocError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RelocError::UnsupportedType { r_type, r_sym } => {
                write!(f, "unsupported reloc type {} sym {}", r_type, r_sym)
            }
            RelocError::SymbolNotFound { sym_name } => {
                write!(f, "symbol not found: {}", sym_name)
            }
            RelocError::TlsError(msg) => write!(f, "TLS error: {}", msg),
            RelocError::CopyRelocNotSupported => write!(f, "COPY relocations not supported"),
            RelocError::TextRelocFailed(msg) => write!(f, "text reloc failed: {}", msg),
            RelocError::BadRelocation(msg) => write!(f, "bad relocation: {}", msg),
        }
    }
}

// ============================================================================
// process_relocation — handles a single reloc entry
//
// This is the Rust equivalent of process_relocation_impl<RelocMode::General>
// from linker_relocate.cpp. It handles all relocation types.
// ============================================================================

struct ProcessRelocArgs<'a> {
    si: &'a SoInfo,
    load_bias: usize,
    base: usize,
    rel_target: *mut u64,
    r_type: u32,
    r_sym: u32,
    r_addend: i64,
    sym_cache: &'a mut SymCache,
    get_symbol: &'a dyn Fn(&str) -> Option<usize>,
}

fn process_relocation(args: ProcessRelocArgs) -> Result<(), RelocError> {
    let ProcessRelocArgs {
        si,
        load_bias,
        base: _base,
        rel_target,
        r_type,
        r_sym,
        r_addend,
        sym_cache,
        get_symbol,
    } = args;

    if r_type == R_GENERIC_NONE {
        return Ok(());
    }

    let sym_name = sym_name_from_soinfo(si, r_sym);

    // --- Symbol resolution ---
    // Determine found_in and sym_addr for this reloc.
    // We handle TLS and non-TLS separately (though on x86_64, TLS relocs also
    // need a symbol lookup).
    let is_tls = is_tls_reloc(r_type);

    let (found_in_base, sym_addr): (usize, usize) = if r_sym == 0 {
        (0, 0)
    } else if is_tls && r_sym != 0 {
        // TLS reloc with a symbol — look it up
        match sym_cache.lookup(r_sym, &sym_name, si, get_symbol) {
            Some((base, val)) => (base, val),
            None => {
                return Err(RelocError::SymbolNotFound {
                    sym_name: sym_name.clone(),
                });
            }
        }
    } else {
        // Non-TLS reloc — standard lookup
        match sym_cache.lookup(r_sym, &sym_name, si, get_symbol) {
            Some((base, val)) => (base, val),
            None => {
                // Weak undefined — allowed, use 0
                (0, 0)
            }
        }
    };

    // --- Dispatch by relocation type ---

    match r_type {
        R_X86_64_RELATIVE => {
            let val = load_bias.wrapping_add(r_addend as usize) as u64;
            unsafe { std::ptr::write(rel_target, val) };
            Ok(())
        }

        R_X86_64_64 => {
            // R_GENERIC_ABSOLUTE
            let val = sym_addr.wrapping_add(r_addend as usize) as u64;
            unsafe { std::ptr::write(rel_target, val) };
            Ok(())
        }

        R_X86_64_GLOB_DAT => {
            let val = sym_addr as u64;
            unsafe { std::ptr::write(rel_target, val) };
            Ok(())
        }

        R_X86_64_JUMP_SLOT => {
            let val = sym_addr as u64;
            unsafe { std::ptr::write(rel_target, val) };
            Ok(())
        }

        R_X86_64_PC32 => {
            let target = sym_addr.wrapping_add(r_addend as usize);
            let base_addr = rel_target as usize;
            let result = target.wrapping_sub(base_addr) as u32;
            unsafe { std::ptr::write(rel_target as *mut u32, result) };
            Ok(())
        }

        R_X86_64_32 => {
            let result = (sym_addr.wrapping_add(r_addend as usize)) as u32;
            unsafe { std::ptr::write(rel_target as *mut u32, result) };
            Ok(())
        }

        R_X86_64_IRELATIVE => {
            let ifunc_addr = load_bias.wrapping_add(r_addend as usize);
            let result = unsafe { call_ifunc_resolver(ifunc_addr) };
            unsafe { std::ptr::write(rel_target, result as u64) };
            Ok(())
        }

        R_X86_64_COPY => {
            Err(RelocError::CopyRelocNotSupported)
        }

        R_X86_64_DTPMOD64 => {
            let module_id = if found_in_base != 0 {
                // Find the module_id for the library containing the symbol
                // We need to look up the TLS module for the library
                find_tls_module_id(si, found_in_base)
            } else {
                0
            };
            unsafe { std::ptr::write(rel_target as *mut u64, module_id as u64) };
            Ok(())
        }

        R_X86_64_DTPOFF64 => {
            let val = sym_addr.wrapping_add(r_addend as usize) as u64;
            unsafe { std::ptr::write(rel_target, val) };
            Ok(())
        }

        R_X86_64_TPOFF64 => {
            // TP-relative offset: sym_value + addend - tls_tp_base
            let tls_tp_base = 0usize; // On Linux, glibc handles this; our TLS layer doesn't track TP base
            let result = sym_addr
                .wrapping_add(r_addend as usize)
                .wrapping_sub(tls_tp_base) as u64;
            unsafe { std::ptr::write(rel_target, result) };
            Ok(())
        }

        R_X86_64_TLSDESC => {
            // TLSDESC is aarch64-only in the C++ implementation
            // On x86_64, it's not commonly used and not implemented here
            Err(RelocError::UnsupportedType { r_type, r_sym })
        }

        _ => Err(RelocError::UnsupportedType { r_type, r_sym }),
    }
}

fn is_tls_reloc(r_type: u32) -> bool {
    matches!(
        r_type,
        R_X86_64_DTPMOD64 | R_X86_64_DTPOFF64 | R_X86_64_TPOFF64 | R_X86_64_TLSDESC
    )
}

fn find_tls_module_id(_si: &SoInfo, _found_in_base: usize) -> usize {
    // Look through TLS modules to find one matching found_in_base
    let tls_modules = tls::get_all_tls_modules();
    for module in tls_modules {
        if let Some(base) = module.soinfo_base {
            if base == _found_in_base {
                return module.module_id;
            }
        }
    }
    0
}

// ============================================================================
// Bulk relocation entry points
// ============================================================================

/// Relocate all relocations for a given soinfo.
/// Returns Ok(()) on success, or Err with the list of errors encountered.
pub fn relocate(
    si: &SoInfo,
    load_bias: usize,
    get_symbol: &dyn Fn(&str) -> Option<usize>,
) -> Result<(), Vec<RelocError>> {
    let base = load_bias;
    let mut sym_cache = SymCache::new();
    let mut errors = Vec::new();

    // Process RELA relocations
    if let Some((rela_addr, rela_size)) = si.rela {
        let count = rela_size / std::mem::size_of::<Rela>();
        let relas = unsafe { std::slice::from_raw_parts(rela_addr as *const Rela, count) };
        for rela in relas {
            let r_type = r_type(rela.r_info);
            let r_sym = r_sym(rela.r_info);
            let rel_target = (base as u64).wrapping_add(rela.r_offset) as *mut u64;

            if let Err(e) = process_relocation(ProcessRelocArgs {
                si,
                load_bias,
                base,
                rel_target,
                r_type,
                r_sym,
                r_addend: rela.r_addend,
                sym_cache: &mut sym_cache,
                get_symbol,
            }) {
                errors.push(e);
            }
        }
    }

    // Process REL relocations (addend from place)
    if let Some((rel_addr, rel_size)) = si.rel {
        let count = rel_size / std::mem::size_of::<Rel>();
        let rels = unsafe { std::slice::from_raw_parts(rel_addr as *const Rel, count) };
        for rel in rels {
            let r_type = r_type(rel.r_info);
            let r_sym = r_sym(rel.r_info);
            let rel_target = (base as u64).wrapping_add(rel.r_offset) as *mut u64;
            // For REL, the addend is in the place (the target location)
            let addend = unsafe { std::ptr::read(rel_target as *const i64) };

            if let Err(e) = process_relocation(ProcessRelocArgs {
                    si,
                    load_bias,
                    base,
                    rel_target,
                    r_type,
                    r_sym,
                    r_addend: addend,
                    sym_cache: &mut sym_cache,
                    get_symbol,
                }) {
                errors.push(e);
            }
        }
    }

    // Process PLT relocations
    if let Some((plt_addr, plt_size)) = si.pltrel {
        let is_rela = si.pltrel_type == crate::soinfo::RelocType::Rela;
        if is_rela {
            let count = plt_size / std::mem::size_of::<Rela>();
            let relas = unsafe { std::slice::from_raw_parts(plt_addr as *const Rela, count) };
            for rela in relas {
                let r_type = r_type(rela.r_info);
                let r_sym = r_sym(rela.r_info);
                let rel_target = (base as u64).wrapping_add(rela.r_offset) as *mut u64;

                if let Err(e) = process_relocation(ProcessRelocArgs {
                    si,
                    load_bias,
                    base,
                    rel_target,
                    r_type,
                    r_sym,
                    r_addend: rela.r_addend,
                    sym_cache: &mut sym_cache,
                    get_symbol,
                }) {
                    errors.push(e);
                }
            }
        } else {
            let count = plt_size / std::mem::size_of::<Rel>();
            let rels = unsafe { std::slice::from_raw_parts(plt_addr as *const Rel, count) };
            for rel in rels {
                let r_type = r_type(rel.r_info);
                let r_sym = r_sym(rel.r_info);
                let rel_target = (base as u64).wrapping_add(rel.r_offset) as *mut u64;
                let addend = unsafe { std::ptr::read(rel_target as *const i64) };

                if let Err(e) = process_relocation(ProcessRelocArgs {
                    si,
                    load_bias,
                    base,
                    rel_target,
                    r_type,
                    r_sym,
                    r_addend: addend,
                    sym_cache: &mut sym_cache,
                    get_symbol,
                }) {
                    errors.push(e);
                }
            }
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::soinfo::{RelocType, SoInfo};

    fn make_test_soinfo(base: usize, size: usize) -> SoInfo {
        SoInfo {
            name: "test.so".into(),
            soname: "test.so".into(),
            base,
            size,
            external_symbols: std::collections::HashMap::new(),
            ..Default::default()
        }
    }

    fn dummy_resolve(_name: &str) -> Option<usize> {
        None
    }

    // --- RelocationKind / Stats ---

    #[test]
    fn test_count_and_reset_stats() {
        reset_linker_stats();
        count_relocation(RelocationKind::Absolute);
        count_relocation(RelocationKind::Symbol);
        count_relocation(RelocationKind::SymbolCached);
        count_relocation(RelocationKind::SymbolCached);
        let counts = unsafe { LINKER_STATS };
        assert_eq!(counts[0], 1);
        assert_eq!(counts[1], 0);
        assert_eq!(counts[2], 1);
        assert_eq!(counts[3], 2);
        reset_linker_stats();
        let counts = unsafe { LINKER_STATS };
        assert_eq!(counts, [0; 4]);
    }

    #[test]
    fn test_print_stats_no_panic() {
        reset_linker_stats();
        print_linker_stats();
    }

    // --- SymCache ---

    #[test]
    fn test_sym_cache_miss_then_hit() {
        let si = make_test_soinfo(0x1000, 0x1000);
        let mut cache = SymCache::new();

        // First lookup — miss
        let resolve = |name: &str| -> Option<usize> {
            match name {
                "foo" => Some(0x5000),
                _ => None,
            }
        };
        let r1 = cache.lookup(1, "foo", &si, &resolve);
        assert_eq!(r1, Some((0x5000, 0x5000)));

        // Second lookup — hit
        let r2 = cache.lookup(1, "foo", &si, &resolve);
        assert_eq!(r2, Some((0x5000, 0x5000)));
        assert_eq!(cache.hits, 1);
    }

    #[test]
    fn test_sym_cache_miss_evicts() {
        let si = make_test_soinfo(0x1000, 0x1000);
        let mut cache = SymCache::new();

        let resolve = |name: &str| -> Option<usize> {
            match name {
                "foo" => Some(0x5000),
                "bar" => Some(0x6000),
                _ => None,
            }
        };

        // First lookup — miss (r_sym=1, "foo")
        cache.lookup(1, "foo", &si, &resolve);
        // Second lookup — miss (r_sym=2, "bar" — different sym, evicts foo)
        cache.lookup(2, "bar", &si, &resolve);
        // Third lookup — miss (r_sym=1, evicted)
        let r = cache.lookup(1, "foo", &si, &resolve);
        assert_eq!(r, Some((0x5000, 0x5000)));
        // No cache hits at all (all were misses)
        assert_eq!(cache.hits, 0);
    }

    // --- resolve_symbol ---

    #[test]
    fn test_resolve_symbol_from_external() {
        let mut si = make_test_soinfo(0x1000, 0x1000);
        si.external_symbols.insert("ext_fn".into(), 0x7000);

        let resolve = |_: &str| -> Option<usize> { panic!("should not be called") };
        let r = resolve_symbol(&si, "ext_fn", &resolve);
        assert_eq!(r, Some((0x7000, 0x7000)));
    }

    #[test]
    fn test_resolve_symbol_from_global() {
        let si = make_test_soinfo(0x1000, 0x1000);
        let resolve = |name: &str| -> Option<usize> {
            assert_eq!(name, "global_fn");
            Some(0x8000)
        };
        let r = resolve_symbol(&si, "global_fn", &resolve);
        assert_eq!(r, Some((0x8000, 0x8000)));
    }

    #[test]
    fn test_resolve_symbol_empty() {
        let si = make_test_soinfo(0x1000, 0x1000);
        let r = resolve_symbol(&si, "", &dummy_resolve);
        assert_eq!(r, None);
    }

    // --- sym_name_from_soinfo ---

    #[test]
    fn test_sym_name_from_soinfo_zero_idx() {
        let si = make_test_soinfo(0x1000, 0x1000);
        assert!(sym_name_from_soinfo(&si, 0).is_empty());
    }

    #[test]
    fn test_sym_name_from_soinfo_no_symtab() {
        let si = make_test_soinfo(0x1000, 0x1000);
        assert!(sym_name_from_soinfo(&si, 1).is_empty());
    }

    #[test]
    fn test_sym_name_from_soinfo_valid() {
        // Build a minimal ELF-like strtab with a symbol entry
        let strtab = b"hello\0world\0";
        let strtab_addr = strtab.as_ptr() as usize;

        // Two dummy symbol entries (48 bytes): st_name (4), st_info (1), st_other (1),
        // st_shndx (2), st_value (8), st_size (8)
        let mut sym_data = vec![0u8; 48];
        // st_name = 6 for first entry (sym_idx=0, points to "world")
        // But we want sym_idx=1 to test non-zero index, so entry 0 has st_name=0, entry 1 has st_name=6
        // Entry 0: st_name=0 (empty string)
        sym_data[0..4].copy_from_slice(&(0u32).to_ne_bytes());
        // Entry 1: st_name=6 (points to "world")
        sym_data[24..28].copy_from_slice(&(6u32).to_ne_bytes());
        let sym_addr = sym_data.as_ptr() as usize;

        let mut si = make_test_soinfo(0x1000, 0x1000);
        si.symtab = Some(sym_addr);
        si.strtab = Some(strtab_addr);
        si.strtab_size = strtab.len();

        let name = sym_name_from_soinfo(&si, 1);
        assert_eq!(name, "world");
    }

    // --- process_relocation (unit tests) ---
    // We test individual reloc types by creating minimal mmap'd memory
    // to serve as the rel_target.

    fn alloc_page() -> *mut u64 {
        let addr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(addr, libc::MAP_FAILED);
        addr as *mut u64
    }

    fn free_page(addr: *mut u64) {
        unsafe { libc::munmap(addr as *mut libc::c_void, 4096) };
    }

    /// Build a minimal SoInfo with a working symtab/strtab for symbol resolution tests.
    /// strtab holds the names consecutively; symtab has one entry per name.
    /// Returns (SoInfo, alloc_base) where alloc_base must be freed by the caller.
    fn make_soinfo_with_sym(names: &[&str]) -> (SoInfo, *mut u64) {
        let page = alloc_page();
        let base = page as usize;
        let page_size = 4096usize;

        // Build strtab: concatenate names with NUL separators
        let mut strtab = Vec::new();
        let mut name_offsets = Vec::new();
        for &name in names {
            name_offsets.push(strtab.len());
            strtab.extend_from_slice(name.as_bytes());
            strtab.push(0);
        }
        // Pad to 8-byte alignment for symtab
        while strtab.len() % 8 != 0 {
            strtab.push(0);
        }
        let strtab_size = strtab.len();
        let strtab_start = 0usize;

        // Copy strtab to the page
        unsafe {
            std::ptr::copy_nonoverlapping(
                strtab.as_ptr(),
                (base + strtab_start) as *mut u8,
                strtab_size,
            );
        }

        // Build symtab: 24 bytes per entry
        let symtab_start = strtab_start + strtab_size;
        let num_syms = names.len();
        let symtab_size = num_syms * 24;
        for (i, &off) in name_offsets.iter().enumerate() {
            let entry_addr = base + symtab_start + i * 24;
            unsafe {
                // st_name (4 bytes)
                std::ptr::write(entry_addr as *mut u32, off as u32);
                // st_info (1 byte): STB_GLOBAL | STT_FUNC = 0x12
                std::ptr::write((entry_addr + 4) as *mut u8, 0x12u8);
                // st_other (1 byte): 0
                std::ptr::write((entry_addr + 5) as *mut u8, 0u8);
                // st_shndx (2 bytes): 1
                std::ptr::write((entry_addr + 6) as *mut u16, 1u16);
                // st_value (8 bytes): 0
                std::ptr::write((entry_addr + 8) as *mut u64, 0u64);
                // st_size (8 bytes): 0
                std::ptr::write((entry_addr + 16) as *mut u64, 0u64);
            }
        }

        let si = SoInfo {
            name: "test.so".into(),
            soname: "test.so".into(),
            base,
            size: page_size,
            symtab: Some(base + symtab_start),
            strtab: Some(base + strtab_start),
            strtab_size,
            external_symbols: std::collections::HashMap::new(),
            ..Default::default()
        };
        (si, page)
    }

    #[test]
    fn test_reloc_none() {
        let target = alloc_page();
        let si = make_test_soinfo(0x1000, 0x1000);
        let mut cache = SymCache::new();
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: 0x1000,
            base: 0x1000,
            rel_target: target,
            r_type: R_GENERIC_NONE,
            r_sym: 0,
            r_addend: 0,
            sym_cache: &mut cache,
            get_symbol: &dummy_resolve,
        });
        assert!(result.is_ok());
        free_page(target);
    }

    #[test]
    fn test_reloc_relative() {
        let target = alloc_page();
        let si = make_test_soinfo(0x1000, 0x1000);
        let mut cache = SymCache::new();
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: 0x1000,
            base: 0x1000,
            rel_target: target,
            r_type: R_X86_64_RELATIVE,
            r_sym: 0,
            r_addend: 0x200,
            sym_cache: &mut cache,
            get_symbol: &dummy_resolve,
        });
        assert!(result.is_ok());
        let val = unsafe { target.read() };
        assert_eq!(val, 0x1200);
        free_page(target);
    }

    #[test]
    fn test_reloc_absolute() {
        let target = alloc_page();
        let (si, si_page) = make_soinfo_with_sym(&["test_sym"]);
        let mut cache = SymCache::new();
        let resolve = |_: &str| -> Option<usize> { Some(0x5000) };
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: si.base,
            base: si.base,
            rel_target: target,
            r_type: R_X86_64_64,
            r_sym: 1,
            r_addend: 0x100,
            sym_cache: &mut cache,
            get_symbol: &resolve,
        });
        assert!(result.is_ok());
        let val = unsafe { target.read() };
        assert_eq!(val, 0x5100);
        free_page(target);
        free_page(si_page);
    }

    #[test]
    fn test_reloc_glob_dat() {
        let target = alloc_page();
        let (si, si_page) = make_soinfo_with_sym(&["test_sym"]);
        let mut cache = SymCache::new();
        let resolve = |_: &str| -> Option<usize> { Some(0x3000) };
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: si.base,
            base: si.base,
            rel_target: target,
            r_type: R_X86_64_GLOB_DAT,
            r_sym: 1,
            r_addend: 0,
            sym_cache: &mut cache,
            get_symbol: &resolve,
        });
        assert!(result.is_ok());
        let val = unsafe { target.read() };
        assert_eq!(val, 0x3000);
        free_page(target);
        free_page(si_page);
    }

    #[test]
    fn test_reloc_jump_slot() {
        let target = alloc_page();
        let (si, si_page) = make_soinfo_with_sym(&["test_sym"]);
        let mut cache = SymCache::new();
        let resolve = |_: &str| -> Option<usize> { Some(0x4000) };
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: si.base,
            base: si.base,
            rel_target: target,
            r_type: R_X86_64_JUMP_SLOT,
            r_sym: 1,
            r_addend: 0,
            sym_cache: &mut cache,
            get_symbol: &resolve,
        });
        assert!(result.is_ok());
        let val = unsafe { target.read() };
        assert_eq!(val, 0x4000);
        free_page(target);
        free_page(si_page);
    }

    #[test]
    fn test_reloc_pc32() {
        let target = alloc_page();
        let (si, si_page) = make_soinfo_with_sym(&["test_sym"]);
        let mut cache = SymCache::new();
        let resolve = |_: &str| -> Option<usize> { Some(0x2000) };
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: si.base,
            base: si.base,
            rel_target: target,
            r_type: R_X86_64_PC32,
            r_sym: 1,
            r_addend: 0x50,
            sym_cache: &mut cache,
            get_symbol: &resolve,
        });
        assert!(result.is_ok());
        let val = unsafe { target.read() as u32 };
        let target_addr = target as usize;
        let expected = (0x2050usize).wrapping_sub(target_addr) as u32;
        assert_eq!(val, expected);
        free_page(target);
        free_page(si_page);
    }

    #[test]
    fn test_reloc_32() {
        let target = alloc_page();
        let (si, si_page) = make_soinfo_with_sym(&["test_sym"]);
        let mut cache = SymCache::new();
        let resolve = |_: &str| -> Option<usize> { Some(0x2000) };
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: si.base,
            base: si.base,
            rel_target: target,
            r_type: R_X86_64_32,
            r_sym: 1,
            r_addend: 0x100,
            sym_cache: &mut cache,
            get_symbol: &resolve,
        });
        assert!(result.is_ok());
        let val = unsafe { target.read() as u32 };
        assert_eq!(val, 0x2100);
        free_page(target);
        free_page(si_page);
    }

    #[test]
    fn test_reloc_copy_not_supported() {
        let target = alloc_page();
        let si = make_test_soinfo(0x1000, 0x1000);
        let mut cache = SymCache::new();
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: 0x1000,
            base: 0x1000,
            rel_target: target,
            r_type: R_X86_64_COPY,
            r_sym: 0,
            r_addend: 0,
            sym_cache: &mut cache,
            get_symbol: &dummy_resolve,
        });
        assert!(matches!(result, Err(RelocError::CopyRelocNotSupported)));
        free_page(target);
    }

    #[test]
    fn test_reloc_unsupported_type() {
        let target = alloc_page();
        let si = make_test_soinfo(0x1000, 0x1000);
        let mut cache = SymCache::new();
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: 0x1000,
            base: 0x1000,
            rel_target: target,
            r_type: 99, // unknown
            r_sym: 0,
            r_addend: 0,
            sym_cache: &mut cache,
            get_symbol: &dummy_resolve,
        });
        assert!(matches!(result, Err(RelocError::UnsupportedType { .. })));
        free_page(target);
    }

    #[test]
    fn test_reloc_irelative() {
        // IRELATIVE calls an IFUNC resolver. We test with a simple resolver
        // that returns a known address.
        let target = alloc_page();
        unsafe { target.write(0) };

        // Define a simple IFUNC resolver
        unsafe extern "C" fn test_ifunc_resolver() -> *mut std::ffi::c_void {
            0xABCD as *mut std::ffi::c_void
        }

        // We need to write the resolver address into the addend (load_bias + addend)
        let resolver_addr = test_ifunc_resolver as usize;
        let load_bias = resolver_addr; // So that load_bias + addend = resolver_addr
        let addend = 0i64; // Since load_bias = resolver_addr

        let si = make_test_soinfo(resolver_addr, 0x1000);
        let mut cache = SymCache::new();
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias,
            base: resolver_addr,
            rel_target: target,
            r_type: R_X86_64_IRELATIVE,
            r_sym: 0,
            r_addend: 0,
            sym_cache: &mut cache,
            get_symbol: &dummy_resolve,
        });
        assert!(result.is_ok());
        let val = unsafe { target.read() };
        assert_eq!(val, 0xABCD);
        free_page(target);
    }

    #[test]
    fn test_reloc_tls_dtpmod() {
        let target = alloc_page();
        let (si, si_page) = make_soinfo_with_sym(&["tls_sym"]);
        let mut cache = SymCache::new();
        let resolve = |_: &str| -> Option<usize> { Some(0x5000) };
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: si.base,
            base: si.base,
            rel_target: target,
            r_type: R_X86_64_DTPMOD64,
            r_sym: 1,
            r_addend: 0,
            sym_cache: &mut cache,
            get_symbol: &resolve,
        });
        assert!(result.is_ok());
        free_page(target);
        free_page(si_page);
    }

    #[test]
    fn test_reloc_tls_dtprel() {
        let target = alloc_page();
        let (si, si_page) = make_soinfo_with_sym(&["tls_sym"]);
        let mut cache = SymCache::new();
        let resolve = |_: &str| -> Option<usize> { Some(0x5000) };
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: si.base,
            base: si.base,
            rel_target: target,
            r_type: R_X86_64_DTPOFF64,
            r_sym: 1,
            r_addend: 0x100,
            sym_cache: &mut cache,
            get_symbol: &resolve,
        });
        assert!(result.is_ok());
        let val = unsafe { target.read() };
        assert_eq!(val, 0x5100);
        free_page(target);
        free_page(si_page);
    }

    #[test]
    fn test_reloc_tls_tprel() {
        let target = alloc_page();
        let (si, si_page) = make_soinfo_with_sym(&["tls_sym"]);
        let mut cache = SymCache::new();
        let resolve = |_: &str| -> Option<usize> { Some(0x5000) };
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: si.base,
            base: si.base,
            rel_target: target,
            r_type: R_X86_64_TPOFF64,
            r_sym: 1,
            r_addend: 0x200,
            sym_cache: &mut cache,
            get_symbol: &resolve,
        });
        assert!(result.is_ok());
        let val = unsafe { target.read() };
        assert_eq!(val, 0x5200);
        free_page(target);
        free_page(si_page);
    }

    #[test]
    fn test_reloc_tlsdesc_not_supported() {
        let target = alloc_page();
        let si = make_test_soinfo(0x1000, 0x1000);
        let mut cache = SymCache::new();
        let result = process_relocation(ProcessRelocArgs {
            si: &si,
            load_bias: 0x1000,
            base: 0x1000,
            rel_target: target,
            r_type: R_X86_64_TLSDESC,
            r_sym: 0,
            r_addend: 0,
            sym_cache: &mut cache,
            get_symbol: &dummy_resolve,
        });
        assert!(matches!(result, Err(RelocError::UnsupportedType { .. })));
        free_page(target);
    }

    // --- relocate() bulk entry point ---

    #[test]
    fn test_relocate_no_relocs() {
        let si = make_test_soinfo(0x1000, 0x1000);
        let result = relocate(&si, 0x1000, &dummy_resolve);
        assert!(result.is_ok());
    }

    #[test]
    fn test_relocate_with_rela() {
        // Create a dummy soinfo with an in-memory RELA table
        let base_alloc = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(base_alloc, libc::MAP_FAILED);

        // Create a target area within the same page
        let target_offset = 2048usize; // offset within page for reloc target
        let target_addr = (base_alloc as usize) + target_offset;

        // Create a RELA entry pointing to the target
        let rela_addr = base_alloc as usize;
        let rela = Rela {
            r_offset: target_offset as u64, // offset relative to base
            r_info: (0u64 << 32) | (R_X86_64_RELATIVE as u64), // r_sym=0, r_type=RELATIVE
            r_addend: 0x42,
        };
        unsafe {
            std::ptr::write(rela_addr as *mut Rela, rela);
        }

        let si = SoInfo {
            name: "test.so".into(),
            soname: "test.so".into(),
            base: base_alloc as usize,
            size: 4096,
            rela: Some((rela_addr, std::mem::size_of::<Rela>())),
            external_symbols: std::collections::HashMap::new(),
            ..Default::default()
        };

        let result = relocate(&si, base_alloc as usize, &dummy_resolve);
        assert!(result.is_ok());

        // Check the target was written with base + addend
        let val = unsafe { *(target_addr as *const u64) };
        assert_eq!(val, (base_alloc as u64) + 0x42);

        unsafe { libc::munmap(base_alloc, 4096) };
    }

    #[test]
    fn test_relocate_with_rel() {
        let base_alloc = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(base_alloc, libc::MAP_FAILED);

        let target_offset = 1024usize;
        let target_addr = (base_alloc as usize) + target_offset;

        // Pre-write the addend in the place (REL uses in-place addend)
        unsafe { *(target_addr as *mut u64) = 0x77 };

        // Create a REL entry (no r_addend field) — 8-byte aligned offset
        let rel_addr = base_alloc as usize + 512;
        let rel = Rel {
            r_offset: target_offset as u64,
            r_info: (0u64 << 32) | (R_X86_64_RELATIVE as u64),
        };
        unsafe {
            std::ptr::write(rel_addr as *mut Rel, rel);
        }

        let si = SoInfo {
            name: "test.so".into(),
            soname: "test.so".into(),
            base: base_alloc as usize,
            size: 4096,
            rel: Some((rel_addr, std::mem::size_of::<Rel>())),
            external_symbols: std::collections::HashMap::new(),
            ..Default::default()
        };

        let result = relocate(&si, base_alloc as usize, &dummy_resolve);
        assert!(result.is_ok());

        // Check: RELATIVE with addend from place
        let val = unsafe { *(target_addr as *const u64) };
        assert_eq!(val, (base_alloc as u64) + 0x77);

        unsafe { libc::munmap(base_alloc, 4096) };
    }

    #[test]
    fn test_relocate_with_pltrel() {
        let base_alloc = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert_ne!(base_alloc, libc::MAP_FAILED);

        let target_offset = 3000usize;
        let target_addr = (base_alloc as usize) + target_offset;

        // Create a PLT RELA entry (JUMP_SLOT) — 8-byte aligned offset
        let plt_addr = base_alloc as usize + 200;
        let resolve = |name: &str| -> Option<usize> {
            assert_eq!(name, "malloc");
            Some(0xDEAD)
        };

        // Build a symbol table so sym_name_from_soinfo works
        let strtab = b"malloc\0";
        let strtab_addr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        } as *mut u8;
        unsafe {
            std::ptr::copy_nonoverlapping(strtab.as_ptr(), strtab_addr, strtab.len());
        }

        // Symbol: st_name=0, st_info=0x12 (STB_GLOBAL|STT_FUNC), st_other=0, st_shndx=1,
        // st_value=0, st_size=0
        let sym_data: [u8; 24] = [
            0, 0, 0, 0, // st_name = 0
            0x12, // st_info
            0,   // st_other
            1, 0, // st_shndx
            0, 0, 0, 0, 0, 0, 0, 0, // st_value
            0, 0, 0, 0, 0, 0, 0, 0, // st_size
        ];
        let sym_addr = unsafe {
            let p = libc::mmap(
                std::ptr::null_mut(),
                4096,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            ) as *mut u8;
            std::ptr::copy_nonoverlapping(sym_data.as_ptr(), p, sym_data.len());
            p
        } as usize;

        let plt_rela = Rela {
            r_offset: target_offset as u64,
            r_info: (1u64 << 32) | (R_X86_64_JUMP_SLOT as u64),
            r_addend: 0,
        };
        unsafe {
            std::ptr::write(plt_addr as *mut Rela, plt_rela);
        }

        let si = SoInfo {
            name: "test.so".into(),
            soname: "test.so".into(),
            base: base_alloc as usize,
            size: 4096,
            pltrel: Some((plt_addr, std::mem::size_of::<Rela>())),
            pltrel_type: RelocType::Rela,
            symtab: Some(sym_addr),
            strtab: Some(strtab_addr as usize),
            strtab_size: strtab.len(),
            external_symbols: std::collections::HashMap::new(),
            ..Default::default()
        };

        let result = relocate(&si, base_alloc as usize, &resolve);
        assert!(result.is_ok());

        let val = unsafe { *(target_addr as *const u64) };
        assert_eq!(val, 0xDEAD);

        unsafe {
            libc::munmap(base_alloc, 4096);
            libc::munmap(strtab_addr as *mut libc::c_void, 4096);
            libc::munmap(sym_addr as *mut libc::c_void, 4096);
        }
    }

    #[test]
    fn test_relocate_collects_errors() {
        let page = alloc_page();
        let base = page as usize;

        // Create a RELA entry with an unsupported reloc type to trigger an error
        let rela_addr = base + 128; // 8-byte aligned offset
        let rela = Rela {
            r_offset: 0,
            r_info: (0u64 << 32) | 99u64, // r_type=99 (unsupported)
            r_addend: 0,
        };
        unsafe {
            std::ptr::write(rela_addr as *mut Rela, rela);
        }

        let si = SoInfo {
            name: "bad.so".into(),
            soname: "bad.so".into(),
            base,
            size: 4096,
            rela: Some((rela_addr, std::mem::size_of::<Rela>())),
            external_symbols: std::collections::HashMap::new(),
            ..Default::default()
        };

        let result = relocate(&si, base, &dummy_resolve);
        assert!(result.is_err());
        free_page(page);
    }

    // --- IFUNC resolver ---

    #[test]
    fn test_call_ifunc_resolver() {
        unsafe extern "C" fn my_resolver() -> *mut std::ffi::c_void {
            0xCAFE as *mut std::ffi::c_void
        }
        let addr = my_resolver as usize;
        let result = unsafe { call_ifunc_resolver(addr) };
        assert_eq!(result, 0xCAFE);
    }

    // --- is_tls_reloc ---

    #[test]
    fn test_is_tls_reloc_true() {
        assert!(is_tls_reloc(R_X86_64_DTPMOD64));
        assert!(is_tls_reloc(R_X86_64_DTPOFF64));
        assert!(is_tls_reloc(R_X86_64_TPOFF64));
        assert!(is_tls_reloc(R_X86_64_TLSDESC));
    }

    #[test]
    fn test_is_tls_reloc_false() {
        assert!(!is_tls_reloc(R_X86_64_RELATIVE));
        assert!(!is_tls_reloc(R_X86_64_64));
        assert!(!is_tls_reloc(R_X86_64_JUMP_SLOT));
        assert!(!is_tls_reloc(R_GENERIC_NONE));
    }

    // --- r_type / r_sym helpers ---

    #[test]
    fn test_r_type_and_sym() {
        let info: u64 = (42u64 << 32) | 7;
        assert_eq!(r_type(info), 7);
        assert_eq!(r_sym(info), 42);

        let info: u64 = 0;
        assert_eq!(r_type(info), 0);
        assert_eq!(r_sym(info), 0);
    }
}

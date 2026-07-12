use crate::soinfo::{RelocType, SoInfo};

const R_X86_64_RELATIVE: u32 = 8;
const R_X86_64_64: u32 = 1;
const R_X86_64_GLOB_DAT: u32 = 6;
const R_X86_64_JUMP_SLOT: u32 = 7;
const R_X86_64_PC32: u32 = 2;

const R_AARCH64_RELATIVE: u32 = 1027;
const R_AARCH64_ABS64: u32 = 257;
const R_AARCH64_GLOB_DAT: u32 = 1025;
const R_AARCH64_JUMP_SLOT: u32 = 1026;

#[derive(Debug, thiserror::Error)]
pub enum RelocError {
    #[error("Unsupported relocation type: {0}")]
    UnsupportedType(u32),
    #[error("Symbol not found: {0}")]
    SymbolNotFound(String),
}

pub fn apply_relocations(
    soinfo: &SoInfo,
    get_symbol: &dyn Fn(&str) -> Option<usize>,
) -> Result<(), Vec<RelocError>> {
    let mut errors = Vec::new();

    if let Some((rela_addr, rela_size)) = soinfo.rela {
        apply_rela(soinfo, rela_addr, rela_size, get_symbol, &mut errors);
    }
    if let Some((rel_addr, rel_size)) = soinfo.rel {
        apply_rel(soinfo, rel_addr, rel_size, get_symbol, &mut errors);
    }
    if let Some((plt_addr, plt_size)) = soinfo.pltrel {
        match soinfo.pltrel_type {
            RelocType::Rela => apply_rela(soinfo, plt_addr, plt_size, get_symbol, &mut errors),
            RelocType::Rel => apply_rel(soinfo, plt_addr, plt_size, get_symbol, &mut errors),
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

#[repr(C)]
struct Rela {
    r_offset: u64,
    r_info: u64,
    r_addend: i64,
}

#[repr(C)]
struct Rel {
    r_offset: u64,
    r_info: u64,
}

fn apply_rela(
    soinfo: &SoInfo,
    addr: usize,
    size: usize,
    get_symbol: &dyn Fn(&str) -> Option<usize>,
    errors: &mut Vec<RelocError>,
) {
    let _count = size / std::mem::size_of::<Rela>();
    let relas = unsafe { std::slice::from_raw_parts(addr as *const Rela, _count) };

    for (_, rela) in relas.iter().enumerate() {
        let r_type = (rela.r_info & 0xffffffff) as u32;
        let r_sym = (rela.r_info >> 32) as u32;
        let place = (soinfo.base as u64).wrapping_add(rela.r_offset) as usize;
        match r_type {
            R_X86_64_RELATIVE | R_AARCH64_RELATIVE => {
                let val = (soinfo.base as u64).wrapping_add(rela.r_addend as u64);
                unsafe { std::ptr::write(place as *mut u64, val); }
            }
            R_X86_64_64 | R_X86_64_GLOB_DAT | R_AARCH64_ABS64 | R_AARCH64_GLOB_DAT => {
                let sym_val = resolve_sym(soinfo, r_sym, get_symbol);
                if let Some(sv) = sym_val {
                    let val = (sv as u64).wrapping_add(rela.r_addend as u64);
                    unsafe { std::ptr::write(place as *mut u64, val); }
                } else if r_sym != 0 {
                    let sym_name = get_sym_name(soinfo, r_sym);
                    errors.push(RelocError::SymbolNotFound(sym_name));
                }
            }
            R_X86_64_JUMP_SLOT | R_AARCH64_JUMP_SLOT => {
                let sym_val = resolve_sym(soinfo, r_sym, get_symbol);
                if let Some(sv) = sym_val {
                    unsafe { std::ptr::write(place as *mut u64, sv as u64); }
                } else if r_sym != 0 {
                    let sym_name = get_sym_name(soinfo, r_sym);
                    errors.push(RelocError::SymbolNotFound(sym_name));
                }
            }
            R_X86_64_PC32 => {
                let sym_val = resolve_sym(soinfo, r_sym, get_symbol);
                if let Some(sv) = sym_val {
                    let val = (sv as u64).wrapping_add(rela.r_addend as u64).wrapping_sub(place as u64);
                    unsafe { std::ptr::write(place as *mut u32, val as u32); }
                } else if r_sym != 0 {
                    let sym_name = get_sym_name(soinfo, r_sym);
                    errors.push(RelocError::SymbolNotFound(sym_name));
                }
            }
            _ => errors.push(RelocError::UnsupportedType(r_type)),
        }
    }
}

fn apply_rel(
    soinfo: &SoInfo,
    addr: usize,
    size: usize,
    get_symbol: &dyn Fn(&str) -> Option<usize>,
    errors: &mut Vec<RelocError>,
) {
    let count = size / std::mem::size_of::<Rel>();
    let rels = unsafe { std::slice::from_raw_parts(addr as *const Rel, count) };

    for (_, rel) in rels.iter().enumerate() {
        let r_type = (rel.r_info & 0xffffffff) as u32;
        let r_sym = (rel.r_info >> 32) as u32;
        let place = (soinfo.base as u64).wrapping_add(rel.r_offset) as usize;
        let addend = unsafe { std::ptr::read(place as *const i64) };

        match r_type {
            R_X86_64_RELATIVE | R_AARCH64_RELATIVE => {
                let val = (soinfo.base as u64).wrapping_add(addend as u64);
                unsafe { std::ptr::write(place as *mut u64, val); }
            }
            R_X86_64_64 | R_X86_64_GLOB_DAT | R_AARCH64_ABS64 | R_AARCH64_GLOB_DAT => {
                let sym_val = resolve_sym(soinfo, r_sym, get_symbol);
                if let Some(sv) = sym_val {
                    unsafe { std::ptr::write(place as *mut u64, sv as u64); }
                } else if r_sym != 0 {
                    let sym_name = get_sym_name(soinfo, r_sym);
                    errors.push(RelocError::SymbolNotFound(sym_name));
                }
            }
            R_X86_64_JUMP_SLOT | R_AARCH64_JUMP_SLOT => {
                let sym_val = resolve_sym(soinfo, r_sym, get_symbol);
                if let Some(sv) = sym_val {
                    unsafe { std::ptr::write(place as *mut u64, sv as u64); }
                } else if r_sym != 0 {
                    let sym_name = get_sym_name(soinfo, r_sym);
                    errors.push(RelocError::SymbolNotFound(sym_name));
                }
            }
            _ => errors.push(RelocError::UnsupportedType(r_type)),
        }
    }
}

fn resolve_sym(
    soinfo: &SoInfo,
    sym_idx: u32,
    get_symbol: &dyn Fn(&str) -> Option<usize>,
) -> Option<usize> {
    if sym_idx == 0 {
        return None;
    }

    let sym_name = get_sym_name(soinfo, sym_idx);

    if let Some(&addr) = soinfo.external_symbols.get(&sym_name) {
        return Some(addr);
    }

    get_symbol(&sym_name)
}

fn get_sym_name(soinfo: &SoInfo, sym_idx: u32) -> String {
    let symtab = match soinfo.symtab {
        Some(s) => s,
        None => return String::new(),
    };
    let strtab = match soinfo.strtab {
        Some(s) => s,
        None => return String::new(),
    };
    let sym_size = 24usize;
    let sym_offset = sym_idx as usize * sym_size;
    // Bounds check: symbol entry must be within the segment
    if sym_offset + sym_size > soinfo.size {
        return String::new();
    }
    unsafe {
        let sym_ptr = (symtab as *const u8).add(sym_offset) as *const u32;
        let st_name = sym_ptr.read() as usize;
        // Bounds check: string must be within the segment
        if st_name >= soinfo.strtab_size {
            return String::new();
        }
        let name_ptr = (strtab as *const u8).add(st_name);
        let cstr = std::ffi::CStr::from_ptr(name_ptr as *const i8);
        cstr.to_str().unwrap_or("").to_string()
    }
}

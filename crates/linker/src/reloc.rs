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
    #[error("Symbol not found")]
    SymbolNotFound,
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
    let count = size / std::mem::size_of::<Rela>();
    let relas = unsafe { std::slice::from_raw_parts(addr as *const Rela, count) };

    for rela in relas {
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
                    errors.push(RelocError::SymbolNotFound);
                }
            }
            R_X86_64_JUMP_SLOT | R_AARCH64_JUMP_SLOT => {
                let sym_val = resolve_sym(soinfo, r_sym, get_symbol);
                if let Some(sv) = sym_val {
                    unsafe { std::ptr::write(place as *mut u64, sv as u64); }
                } else if r_sym != 0 {
                    errors.push(RelocError::SymbolNotFound);
                }
            }
            R_X86_64_PC32 => {
                let sym_val = resolve_sym(soinfo, r_sym, get_symbol);
                if let Some(sv) = sym_val {
                    let val = (sv as u64).wrapping_add(rela.r_addend as u64).wrapping_sub(place as u64);
                    unsafe { std::ptr::write(place as *mut u32, val as u32); }
                } else if r_sym != 0 {
                    errors.push(RelocError::SymbolNotFound);
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

    for rel in rels {
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
                    errors.push(RelocError::SymbolNotFound);
                }
            }
            R_X86_64_JUMP_SLOT | R_AARCH64_JUMP_SLOT => {
                let sym_val = resolve_sym(soinfo, r_sym, get_symbol);
                if let Some(sv) = sym_val {
                    unsafe { std::ptr::write(place as *mut u64, sv as u64); }
                } else if r_sym != 0 {
                    errors.push(RelocError::SymbolNotFound);
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
    let sym_ptr = soinfo.symtab.map(|s| s as *const u8);
    let str_ptr = soinfo.strtab.map(|s| s as *const u8);
    if let (Some(sp), Some(stp)) = (sym_ptr, str_ptr) {
        unsafe {
            let sym_offset = (sp.add(sym_idx as usize * 24) as *const u32).read() as usize;
            let name_ptr = stp.add(sym_offset);
            let cstr = std::ffi::CStr::from_ptr(name_ptr as *const i8);
            cstr.to_str().unwrap_or("").to_string()
        }
    } else {
        String::new()
    }
}

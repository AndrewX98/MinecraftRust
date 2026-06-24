use crate::soinfo::{RelocType, SoInfo};
use goblin::elf::program_header::ProgramHeader;
use goblin::elf::sym::Sym;
use goblin::elf::Elf;
use goblin::elf;

#[derive(Debug)]
pub struct LoadedElf {
    pub soinfo: SoInfo,
    pub data: Vec<u8>,
}

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("Failed to parse ELF: {0}")]
    Parse(String),
    #[error("No PT_LOAD segments found")]
    NoLoadSegments,
    #[error("Not a shared library")]
    NotSharedLibrary,
}

pub fn load_elf(data: &[u8], name: &str) -> Result<LoadedElf, LoadError> {
    let elf = Elf::parse(data).map_err(|e| LoadError::Parse(format!("{:?}", e)))?;

    let is_lib = elf.header.e_type == elf::header::ET_DYN;
    if !is_lib {
        return Err(LoadError::NotSharedLibrary);
    }

    let mut base: usize = 0;
    let mut total_size: usize = 0;
    let mut phdrs: Vec<ProgramHeader> = Vec::new();

    for phdr in &elf.program_headers {
        if phdr.p_type == elf::program_header::PT_LOAD {
            let end = (phdr.p_vaddr + phdr.p_memsz) as usize;
            total_size = total_size.max(end);
            phdrs.push(phdr.clone());
        }
    }

    if phdrs.is_empty() {
        return Err(LoadError::NoLoadSegments);
    }

    let addr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            total_size,
            libc::PROT_NONE,
            libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
            -1,
            0,
        )
    };
    if addr == libc::MAP_FAILED {
        return Err(LoadError::Parse("mmap failed".to_string()));
    }
    base = addr as usize;

    for phdr in &phdrs {
        let seg_start = base + phdr.p_vaddr as usize;
        let seg_memsz = phdr.p_memsz as usize;
        let prot = phdr_flags_to_prot(phdr.p_flags);

        unsafe {
            libc::mmap(
                seg_start as *mut libc::c_void,
                seg_memsz,
                prot,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED,
                -1,
                0,
            );
        }

        let file_start = phdr.p_offset as usize;
        let file_end = file_start + phdr.p_filesz as usize;
        if file_end <= data.len() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr().add(file_start),
                    seg_start as *mut u8,
                    phdr.p_filesz as usize,
                );
            }
        }
    }

    let mut soinfo = SoInfo {
        name: name.to_string(),
        soname: String::new(),
        base,
        size: total_size,
        external_symbols: std::collections::HashMap::new(),
        ..Default::default()
    };

    let mut dependencies = Vec::new();
    let mut strtab: Option<u64> = None;
    let mut strtab_size: usize = 0;
    let mut symtab: Option<u64> = None;
    let mut pltrel: Option<(u64, u64)> = None;
    let mut pltrel_type = RelocType::Rela;
    let mut rela: Option<(u64, u64)> = None;
    let mut rel: Option<(u64, u64)> = None;
    let mut init_fn: Option<u64> = None;
    let mut init_array: Option<(u64, u64)> = None;
    let mut fini_fn: Option<u64> = None;
    let mut fini_array: Option<(u64, u64)> = None;
    let mut preinit_array: Option<(u64, u64)> = None;
    let mut soname_idx: Option<u64> = None;

    if let Some(ref dynamic) = elf.dynamic {
        for entry in &dynamic.dyns {
            match entry.d_tag {
                elf::dynamic::DT_STRTAB => strtab = Some(entry.d_val),
                elf::dynamic::DT_STRSZ => strtab_size = entry.d_val as usize,
                elf::dynamic::DT_SYMTAB => symtab = Some(entry.d_val),
                elf::dynamic::DT_PLTREL => {
                    pltrel_type = if entry.d_val as u64 == elf::dynamic::DT_REL as u64 {
                        RelocType::Rel
                    } else {
                        RelocType::Rela
                    };
                }
                elf::dynamic::DT_JMPREL => pltrel = Some((entry.d_val, 0)),
                elf::dynamic::DT_PLTRELSZ => {
                    if let Some((off, _)) = pltrel.take() {
                        pltrel = Some((off, entry.d_val));
                    }
                }
                elf::dynamic::DT_RELA => rela = Some((entry.d_val, 0)),
                elf::dynamic::DT_RELASZ => {
                    if let Some((off, _)) = rela.take() {
                        rela = Some((off, entry.d_val));
                    }
                }
                elf::dynamic::DT_REL => rel = Some((entry.d_val, 0)),
                elf::dynamic::DT_RELSZ => {
                    if let Some((off, _)) = rel.take() {
                        rel = Some((off, entry.d_val));
                    }
                }
                elf::dynamic::DT_INIT => init_fn = Some(entry.d_val),
                elf::dynamic::DT_INIT_ARRAY => init_array = Some((entry.d_val, 0)),
                elf::dynamic::DT_INIT_ARRAYSZ => {
                    if let Some((off, _)) = init_array.take() {
                        init_array = Some((off, entry.d_val));
                    }
                }
                elf::dynamic::DT_FINI => fini_fn = Some(entry.d_val),
                elf::dynamic::DT_FINI_ARRAY => fini_array = Some((entry.d_val, 0)),
                elf::dynamic::DT_FINI_ARRAYSZ => {
                    if let Some((off, _)) = fini_array.take() {
                        fini_array = Some((off, entry.d_val));
                    }
                }
                elf::dynamic::DT_PREINIT_ARRAY => preinit_array = Some((entry.d_val, 0)),
                elf::dynamic::DT_PREINIT_ARRAYSZ => {
                    if let Some((off, _)) = preinit_array.take() {
                        preinit_array = Some((off, entry.d_val));
                    }
                }
                elf::dynamic::DT_NEEDED => {
                    dependencies.push(entry.d_val);
                }
                elf::dynamic::DT_SONAME => soname_idx = Some(entry.d_val),
                _ => {}
            }
        }
    }

    // Resolve DT_NEEDED names from strtab
    let resolved_deps: Vec<String> = if let (Some(st), sz) = (strtab, strtab_size) {
        let strtab_base = base + st as usize;
        dependencies
            .iter()
            .map(|&off| {
                let ptr = strtab_base + off as usize;
                let cstr = unsafe { std::ffi::CStr::from_ptr(ptr as *const i8) };
                cstr.to_str().unwrap_or("").to_string()
            })
            .collect()
    } else {
        Vec::new()
    };

    let soname = if let Some(idx) = soname_idx {
        if let Some(st) = strtab {
            let ptr = base + st as usize + idx as usize;
            let cstr = unsafe { std::ffi::CStr::from_ptr(ptr as *const i8) };
            cstr.to_str().unwrap_or("").to_string()
        } else {
            String::new()
        }
    } else {
        String::new()
    };

    let strtab_abs = strtab.map(|s| base + s as usize);
    let symtab_abs = symtab.map(|s| base + s as usize);

    soinfo.soname = if soname.is_empty() { name.to_string() } else { soname };
    soinfo.symtab = symtab_abs;
    soinfo.symtab_size = strtab_size;
    soinfo.strtab = strtab_abs;
    soinfo.strtab_size = strtab_size;
    soinfo.dependencies = resolved_deps;
    soinfo.init = init_fn.map(|v| base + v as usize);
    soinfo.init_array = init_array.map(|(a, s)| (base + a as usize, s as usize));
    soinfo.fini = fini_fn.map(|v| base + v as usize);
    soinfo.fini_array = fini_array.map(|(a, s)| (base + a as usize, s as usize));
    soinfo.preinit_array = preinit_array.map(|(a, s)| (base + a as usize, s as usize));
    soinfo.pltrel = pltrel.map(|(o, s)| (base + o as usize, s as usize));
    soinfo.pltrel_type = pltrel_type;
    soinfo.rel = rel.map(|(o, s)| (base + o as usize, s as usize));
    soinfo.rela = rela.map(|(o, s)| (base + o as usize, s as usize));

    Ok(LoadedElf {
        soinfo,
        data: data.to_vec(),
    })
}

fn phdr_flags_to_prot(flags: u32) -> i32 {
    let mut prot = 0;
    if flags & elf::program_header::PF_R != 0 {
        prot |= libc::PROT_READ;
    }
    if flags & elf::program_header::PF_W != 0 {
        prot |= libc::PROT_WRITE;
    }
    if flags & elf::program_header::PF_X != 0 {
        prot |= libc::PROT_EXEC;
    }
    prot
}

use crate::soinfo::SoInfo;
use goblin::elf::Sym;

fn gnu_hash(name: &str) -> u32 {
    let bytes = name.as_bytes();
    let mut h: u32 = 5381;
    for &b in bytes {
        h = h.wrapping_mul(33).wrapping_add(b as u32);
    }
    h
}

fn sysv_hash(name: &str) -> u32 {
    let bytes = name.as_bytes();
    let mut h: u32 = 0;
    for &b in bytes {
        h = h.wrapping_mul(65599).wrapping_add(b as u32);
    }
    h
}

pub fn find_symbol<'a>(soinfo: &'a SoInfo, name: &'a str) -> Option<(usize, &'a [u8])> {
    if !soinfo.gnu_chain.is_empty() {
        find_symbol_gnu(soinfo, name)
    } else if !soinfo.chain.is_empty() {
        find_symbol_sysv(soinfo, name)
    } else {
        None
    }
}

fn find_symbol_gnu<'a>(soinfo: &'a SoInfo, name: &str) -> Option<(usize, &'a [u8])> {
    let h = gnu_hash(name);
    let n = soinfo.gnu_bloom_n;
    if n == 0 {
        return None;
    }

    let bloom_idx = (h / (std::mem::size_of::<usize>() as u32 * 8)) as usize % n;
    let bit = h % (std::mem::size_of::<usize>() as u32 * 8);
    let bit2 = (h >> soinfo.gnu_bloom_shift as u32) % (std::mem::size_of::<usize>() as u32 * 8);

    let word = soinfo.gnu_bloom_filter.get(bloom_idx).copied().unwrap_or(0);
    if (word >> bit) & 1 == 0 || (word >> bit2) & 1 == 0 {
        return None;
    }

    let gnu_bucket = &soinfo.gnu_bucket;
    let gnu_chain = &soinfo.gnu_chain;

    let bucket_idx = (h % gnu_bucket.len() as u32) as usize;
    let mut sym_idx = gnu_bucket.get(bucket_idx).copied().unwrap_or(0) as usize;
    if sym_idx == 0 {
        return None;
    }

    loop {
        let chain_val = gnu_chain.get(sym_idx - soinfo.gnu_bucket.len()).copied().unwrap_or(0);
        if (chain_val | 1) == (h | 1) {
            if let Some(strtab_bytes) = soinfo.strtab.map(|s| unsafe {
                let ptr = s as *const u8;
                let len = soinfo.strtab_size;
                std::slice::from_raw_parts(ptr, len)
            }) {
                if let Some(sym) = soinfo.symtab.map(|s| unsafe {
                    let ptr = s as *const Sym;
                    &*ptr.add(sym_idx)
                }) {
                    let sym_name_ptr = strtab_bytes.as_ptr() as usize + sym.st_name as usize;
                    let sym_name = unsafe {
                        let cstr = std::ffi::CStr::from_ptr(sym_name_ptr as *const i8);
                        cstr.to_str().unwrap_or("")
                    };
                    if sym_name == name && sym.st_shndx != 0 {
                        let value = if sym.st_value != 0 {
                            soinfo.base + sym.st_value as usize
                        } else {
                            0
                        };
                        return Some((value, strtab_bytes));
                    }
                }
            }
        }
        if chain_val & 1 != 0 {
            break;
        }
        sym_idx += 1;
    }

    None
}

fn find_symbol_sysv<'a>(soinfo: &'a SoInfo, name: &str) -> Option<(usize, &'a [u8])> {
    let h = sysv_hash(name);
    let bucket_count = soinfo.bucket_count;
    if bucket_count == 0 {
        return None;
    }

    let bucket = &soinfo.bucket;
    let chain = &soinfo.chain;

    let bucket_idx = (h % bucket_count as u32) as usize;
    let mut sym_idx = bucket.get(bucket_idx).copied().unwrap_or(0) as usize;
    if sym_idx == 0 {
        return None;
    }

    loop {
        if let Some(strtab_bytes) = soinfo.strtab.map(|s| unsafe {
            let ptr = s as *const u8;
            let len = soinfo.strtab_size;
            std::slice::from_raw_parts(ptr, len)
        }) {
            if let Some(sym) = soinfo.symtab.map(|s| unsafe {
                let ptr = s as *const Sym;
                &*ptr.add(sym_idx)
            }) {
                let sym_name_ptr = strtab_bytes.as_ptr() as usize + sym.st_name as usize;
                let sym_name = unsafe {
                    let cstr = std::ffi::CStr::from_ptr(sym_name_ptr as *const i8);
                    cstr.to_str().unwrap_or("")
                };
                if sym_name == name && sym.st_shndx != 0 {
                    let value = if sym.st_value != 0 {
                        soinfo.base + sym.st_value as usize
                    } else {
                        0
                    };
                    return Some((value, strtab_bytes));
                }
            }
        }

        let chain_val = chain.get(sym_idx).copied().unwrap_or(0) as u32;
        if (chain_val & 0xff) == 0 {
            break;
        }
        sym_idx = chain_val as usize;
        if sym_idx == 0 {
            break;
        }
    }

    None
}

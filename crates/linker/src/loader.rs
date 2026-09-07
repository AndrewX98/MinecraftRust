use crate::soinfo::{RelocType, SoInfo, TlsSegment};
use goblin::elf;

#[derive(Clone, Debug)]
struct Phdr {
    p_type: u32,
    p_flags: u32,
    p_offset: u64,
    p_vaddr: u64,
    p_filesz: u64,
    p_memsz: u64,
    p_align: u64,
}
struct DynEnt {
    d_tag: i64,
    d_val: u64,
}

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
    load_elf_inner(data, None, name)
}

pub fn load_elf_with_fd(fd: i32, data: &[u8], name: &str) -> Result<LoadedElf, LoadError> {
    load_elf_inner(data, Some(fd), name)
}

fn load_elf_inner(data: &[u8], fd: Option<i32>, name: &str) -> Result<LoadedElf, LoadError> {
    #[cfg(feature = "perf")]
    let t0 = std::time::Instant::now();
    // Manual ELF header/PHDR parse — zero alloc, no goblin SHT walk, no extra copies.
    // This replaces goblin::Elf::parse which faults all pages and allocates Vecs.
    let (e_type, phdrs_raw, dyn_phdr_raw) = parse_elf_headers(data)?;
    #[cfg(feature = "perf")]
    {
        let dt = t0.elapsed();
        if data.len() > 5 * 1024 * 1024 || name.contains("minecraftpe") {
            eprintln!("[PERF] span label=goblin_parse:{} ms={} us={}", name, dt.as_millis(), dt.as_micros());
        }
    }

    if e_type != elf::header::ET_DYN {
        return Err(LoadError::NotSharedLibrary);
    }

    let mut base: usize = 0;
    let mut total_size: usize = 0;
    let mut phdrs: Vec<Phdr> = Vec::new();
    let mut tls_phdr: Option<Phdr> = None;
    let mut relro_phdr: Option<Phdr> = None;
    let mut dyn_phdr: Option<Phdr> = dyn_phdr_raw.clone();

    for phdr in &phdrs_raw {
        match phdr.p_type {
            elf::program_header::PT_LOAD => {
                let end = (phdr.p_vaddr + phdr.p_memsz) as usize;
                total_size = total_size.max(end);
                phdrs.push(phdr.clone());
            }
            elf::program_header::PT_TLS => {
                tls_phdr = Some(phdr.clone());
            }
            elf::program_header::PT_GNU_RELRO => {
                relro_phdr = Some(phdr.clone());
            }
            elf::program_header::PT_DYNAMIC => {
                // already captured as dyn_phdr_raw
            }
            _ => {}
        }
    }
    // Ensure dyn_phdr is set if not captured via clone above (fallback)
    if dyn_phdr.is_none() {
        for ph in &phdrs_raw {
            if ph.p_type == elf::program_header::PT_DYNAMIC {
                dyn_phdr = Some(ph.clone());
                break;
            }
        }
    }

    if phdrs.is_empty() {
        return Err(LoadError::NoLoadSegments);
    }

    let page_size = 0x1000;
    let aligned_total = (total_size + page_size - 1) & !(page_size - 1);
    let addr = unsafe {
        libc::mmap(
            std::ptr::null_mut(),
            aligned_total,
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

    let mut load_segments: Vec<(usize, usize, i32)> = Vec::new();

    for phdr in &phdrs {
        let seg_start = base + phdr.p_vaddr as usize;
        let seg_memsz = phdr.p_memsz as usize;
        let seg_filesz = phdr.p_filesz as usize;
        let file_off = phdr.p_offset as usize;
        let final_prot = phdr_flags_to_prot(phdr.p_flags);
        load_segments.push((seg_start, seg_memsz, final_prot));
        let map_prot = final_prot | libc::PROT_WRITE;

        let use_file_mmap = fd.is_some() && seg_filesz > 0;
        if use_file_mmap {
            let fd_val = fd.unwrap();
            let seg_page_start = seg_start & !(page_size - 1);
            let seg_page_end = (seg_start + seg_memsz + page_size - 1) & !(page_size - 1);
            let seg_file_end = seg_start + seg_filesz;
            let file_page_start = file_off & !(page_size - 1);
            let file_length = (file_off + seg_filesz) - file_page_start;
            if file_length > 0 {
                let r = unsafe {
                    libc::mmap(
                        seg_page_start as *mut libc::c_void,
                        file_length,
                        map_prot,
                        libc::MAP_PRIVATE | libc::MAP_FIXED,
                        fd_val,
                        file_page_start as i64,
                    )
                };
                if r != libc::MAP_FAILED {
                    if (phdr.p_flags & 0x2) != 0 && (seg_file_end & (page_size - 1)) != 0 {
                        unsafe {
                            let tail = seg_file_end as *mut u8;
                            let len = page_size - (seg_file_end & (page_size - 1));
                            std::ptr::write_bytes(tail, 0, len);
                        }
                    }
                    let seg_file_end_page = (seg_file_end + page_size - 1) & !(page_size - 1);
                    if seg_page_end > seg_file_end_page {
                        let anon_size = seg_page_end - seg_file_end_page;
                        unsafe {
                            libc::mmap(
                                seg_file_end_page as *mut libc::c_void,
                                anon_size,
                                map_prot,
                                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED,
                                -1,
                                0,
                            );
                        }
                    }
                    continue;
                }
            }
        }
        // Fallback: anonymous + copy
        let aligned_start = seg_start & !(page_size - 1);
        let offset_in_page = seg_start - aligned_start;
        let aligned_size = seg_memsz + offset_in_page;

        unsafe {
            let r = libc::mmap(
                aligned_start as *mut libc::c_void,
                aligned_size,
                map_prot,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED,
                -1,
                0,
            );
            if r == libc::MAP_FAILED {
                continue;
            }
        }

        let file_end = file_off + seg_filesz;
        if file_end <= data.len() {
            unsafe {
                std::ptr::copy_nonoverlapping(
                    data.as_ptr().add(file_off),
                    seg_start as *mut u8,
                    seg_filesz,
                );
            }
        }
    }

    let dynamic_addr = dyn_phdr.as_ref().map(|ph| (base + ph.p_vaddr as usize) as usize);

    let mut soinfo = SoInfo {
        name: name.to_string(),
        soname: String::new(),
        base,
        size: total_size,
        dynamic: dynamic_addr,
        external_symbols: std::collections::HashMap::new(),
        ..Default::default()
    };

    let mut dependencies = Vec::new();
    let mut strtab: Option<u64> = None;
    let mut strtab_size: usize = 0;
    let mut symtab: Option<u64> = None;
    let mut gnu_hash_addr: Option<u64> = None;
    let mut sysv_hash_addr: Option<u64> = None;
    let mut pltrel_off: Option<u64> = None;
    let mut pltrel_sz: Option<u64> = None;
    let mut rela_off: Option<u64> = None;
    let mut rela_sz: Option<u64> = None;
    let mut rel_off: Option<u64> = None;
    let mut rel_sz: Option<u64> = None;
    // Order-independent offset/size pairs (DT_PLTRELSZ can precede DT_JMPREL
    // in glibc-built objects).
    let mut init_array_off: Option<u64> = None;
    let mut init_array_sz: Option<u64> = None;
    let mut fini_array_off: Option<u64> = None;
    let mut fini_array_sz: Option<u64> = None;
    let mut preinit_array_off: Option<u64> = None;
    let mut preinit_array_sz: Option<u64> = None;
    let mut init_fn: Option<u64> = None;
    let mut fini_fn: Option<u64> = None;
    let mut pltrel_type = RelocType::Rela;
    let mut soname_idx: Option<u64> = None;

    // Manual DT walk from file data (PT_DYNAMIC payload)
    {
        let dyn_ents: Vec<DynEnt> = if let Some(ref dph) = dyn_phdr {
            let off = dph.p_offset as usize;
            let sz = dph.p_filesz as usize;
            if off + sz <= data.len() && sz % 16 == 0 {
                let mut v = Vec::with_capacity(sz / 16);
                for chunk in data[off..off + sz].chunks_exact(16) {
                    let d_tag = i64::from_le_bytes(chunk[0..8].try_into().unwrap());
                    let d_val = u64::from_le_bytes(chunk[8..16].try_into().unwrap());
                    if d_tag == elf::dynamic::DT_NULL as i64 {
                        v.push(DynEnt { d_tag, d_val });
                        break;
                    }
                    v.push(DynEnt { d_tag, d_val });
                }
                v
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };
        for entry in dyn_ents {
            match entry.d_tag as i64 {
                x if x == elf::dynamic::DT_STRTAB as i64 => strtab = Some(entry.d_val),
                x if x == elf::dynamic::DT_STRSZ as i64 => strtab_size = entry.d_val as usize,
                x if x == elf::dynamic::DT_SYMTAB as i64 => symtab = Some(entry.d_val),
                x if x == elf::dynamic::DT_PLTREL as i64 => {
                    pltrel_type = if entry.d_val == elf::dynamic::DT_REL as u64 {
                        RelocType::Rel
                    } else {
                        RelocType::Rela
                    };
                }
                x if x == elf::dynamic::DT_JMPREL as i64 => pltrel_off = Some(entry.d_val),
                x if x == elf::dynamic::DT_PLTRELSZ as i64 => pltrel_sz = Some(entry.d_val),
                x if x == elf::dynamic::DT_RELA as i64 => rela_off = Some(entry.d_val),
                x if x == elf::dynamic::DT_RELASZ as i64 => rela_sz = Some(entry.d_val),
                x if x == elf::dynamic::DT_REL as i64 => rel_off = Some(entry.d_val),
                x if x == elf::dynamic::DT_RELSZ as i64 => rel_sz = Some(entry.d_val),
                x if x == elf::dynamic::DT_INIT as i64 => init_fn = Some(entry.d_val),
                x if x == elf::dynamic::DT_INIT_ARRAY as i64 => init_array_off = Some(entry.d_val),
                x if x == elf::dynamic::DT_INIT_ARRAYSZ as i64 => init_array_sz = Some(entry.d_val),
                x if x == elf::dynamic::DT_FINI as i64 => fini_fn = Some(entry.d_val),
                x if x == elf::dynamic::DT_FINI_ARRAY as i64 => fini_array_off = Some(entry.d_val),
                x if x == elf::dynamic::DT_FINI_ARRAYSZ as i64 => fini_array_sz = Some(entry.d_val),
                x if x == elf::dynamic::DT_PREINIT_ARRAY as i64 => preinit_array_off = Some(entry.d_val),
                x if x == elf::dynamic::DT_PREINIT_ARRAYSZ as i64 => preinit_array_sz = Some(entry.d_val),
                x if x == elf::dynamic::DT_NEEDED as i64 => dependencies.push(entry.d_val),
                x if x == elf::dynamic::DT_SONAME as i64 => soname_idx = Some(entry.d_val),
                x if x == elf::dynamic::DT_GNU_HASH as i64 => gnu_hash_addr = Some(entry.d_val),
                x if x == elf::dynamic::DT_HASH as i64 => sysv_hash_addr = Some(entry.d_val),
                _ => {}
            }
        }
    }

    // Resolve DT_NEEDED names from strtab
    let resolved_deps: Vec<String> = if let (Some(st), _sz) = (strtab, strtab_size) {
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
    let pltrel: Option<(u64, u64)> = match (pltrel_off, pltrel_sz) {
        (Some(o), Some(s)) => Some((o, s)),
        _ => None,
    };
    let rela: Option<(u64, u64)> = match (rela_off, rela_sz) {
        (Some(o), Some(s)) => Some((o, s)),
        _ => None,
    };
    let rel: Option<(u64, u64)> = match (rel_off, rel_sz) {
        (Some(o), Some(s)) => Some((o, s)),
        _ => None,
    };
    let init_array: Option<(u64, u64)> = match (init_array_off, init_array_sz) {
        (Some(o), Some(s)) => Some((o, s)),
        _ => None,
    };
    let fini_array: Option<(u64, u64)> = match (fini_array_off, fini_array_sz) {
        (Some(o), Some(s)) => Some((o, s)),
        _ => None,
    };
    let preinit_array: Option<(u64, u64)> = match (preinit_array_off, preinit_array_sz) {
        (Some(o), Some(s)) => Some((o, s)),
        _ => None,
    };
    soinfo.init_array = init_array.map(|(a, s)| (base + a as usize, s as usize));
    soinfo.fini = fini_fn.map(|v| base + v as usize);
    soinfo.fini_array = fini_array.map(|(a, s)| (base + a as usize, s as usize));
    soinfo.preinit_array = preinit_array.map(|(a, s)| (base + a as usize, s as usize));
    soinfo.pltrel = pltrel.map(|(o, s)| (base + o as usize, s as usize));
    soinfo.pltrel_type = pltrel_type;
    soinfo.rel = rel.map(|(o, s)| (base + o as usize, s as usize));
    soinfo.rela = rela.map(|(o, s)| (base + o as usize, s as usize));

    // Parse hash tables from .dynamic section (DT_GNU_HASH / DT_HASH)
    if let Some(gnu_off) = gnu_hash_addr {
        let gnu_hash_ptr = base + gnu_off as usize;
        soinfo.gnu_hash = Some(gnu_hash_ptr);
        unsafe {
            let header = *(gnu_hash_ptr as *const [u32; 4]);
            let nbuckets = header[0] as usize;
            let symoffset = header[1] as usize;
            let bloom_size = header[2] as usize;
            let bloom_shift = header[3];
            if bloom_size > 0 && bloom_size.is_power_of_two() {
                let bloom_filter_ptr = (gnu_hash_ptr as *const u8).add(16) as *const usize;
                let buckets_ptr = bloom_filter_ptr.add(bloom_size) as *const u32;
                let bloom_filter =
                    std::slice::from_raw_parts(bloom_filter_ptr, bloom_size);
                let buckets = std::slice::from_raw_parts(buckets_ptr, nbuckets);

                // Compute dynsym count from hash section size.
                // .gnu.hash and .dynstr are adjacent in the ELF, so:
                //   hash_section_size = strtab_vaddr - gnu_hash_vaddr
                //   chains_len = (hash_section_size - 16 - bloom_size*8 - nbuckets*4) / 4
                //   dynsym_count = symoffset + chains_len
                let dynsym_count = if let (Some(gh), Some(st)) = (gnu_hash_addr, strtab) {
                    if st > gh {
                        let hash_section_size = (st - gh) as usize;
                        let chains_data = hash_section_size
                            .saturating_sub(16 + bloom_size * 8 + nbuckets * 4);
                        let chains_len = chains_data / 4;
                        symoffset + chains_len
                    } else {
                        // Fallback: approximate from second-to-last bucket chain end
                        symoffset
                    }
                } else {
                    symoffset
                };
                let chains_len = if dynsym_count > symoffset {
                    dynsym_count - symoffset
                } else {
                    0
                };
                let chains = if chains_len > 0 {
                    let chains_ptr = buckets_ptr.add(nbuckets);
                    std::slice::from_raw_parts(chains_ptr, chains_len)
                } else {
                    &[]
                };

                soinfo.gnu_symoffset = symoffset;
                soinfo.gnu_bloom_filter = bloom_filter.to_vec();
                soinfo.gnu_bloom_shift = bloom_shift as usize;
                soinfo.gnu_bloom_n = bloom_size;
                soinfo.gnu_bucket = buckets.to_vec();
                soinfo.gnu_chain = chains.to_vec();
                soinfo.dynsym_count = dynsym_count;
                soinfo.set_gnu_hash_flag();
            }
        }
    } else if let Some(sysv_off) = sysv_hash_addr {
        let sysv_hash_ptr = base + sysv_off as usize;
        soinfo.sysv_hash = Some(sysv_hash_ptr);
        unsafe {
            let nbuckets = *(sysv_hash_ptr as *const u32);
            let nchains = *((sysv_hash_ptr as *const u32).add(1));
            if nbuckets > 0 && nchains > 0 {
                let buckets_ptr = (sysv_hash_ptr as *const u32).add(2);
                let chains_ptr = buckets_ptr.add(nbuckets as usize);
                let buckets = std::slice::from_raw_parts(buckets_ptr, nbuckets as usize);
                let chains = std::slice::from_raw_parts(chains_ptr, nchains as usize);
                soinfo.bucket_count = nbuckets as usize;
                soinfo.bucket = buckets.to_vec();
                soinfo.chain = chains.to_vec();
                soinfo.dynsym_count = nchains as usize;
            }
        }
    }

    if let Some(tls) = tls_phdr {
        soinfo.tls_segment = Some(TlsSegment {
            size: tls.p_memsz as usize,
            alignment: tls.p_align as usize,
            init_ptr: base + tls.p_vaddr as usize,
            init_size: tls.p_filesz as usize,
        });
    }

    if let Some(relro) = relro_phdr {
        soinfo.pt_gnu_relro = Some((base + relro.p_vaddr as usize, relro.p_memsz as usize));
    }

    soinfo.load_segments = load_segments;

    Ok(LoadedElf {
        soinfo,
        data: Vec::new(),
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

fn parse_elf_headers(data: &[u8]) -> Result<(u16, Vec<Phdr>, Option<Phdr>), LoadError> {
    if data.len() < 64 {
        return Err(LoadError::Parse("ELF too small".into()));
    }
    if data[0] != 0x7f || data[1] != b'E' || data[2] != b'L' || data[3] != b'F' {
        return Err(LoadError::Parse("bad ELF magic".into()));
    }
    let ei_class = data[4];
    let ei_data = data[5];
    if ei_class != 2 {
        return Err(LoadError::Parse("only ELF64 supported".into()));
    }
    if ei_data != 1 && ei_data != 2 {
        return Err(LoadError::Parse("unknown EI_DATA".into()));
    }
    let le = ei_data == 1;
    let read_u16 = |off: usize| {
        let b: [u8; 2] = data[off..off + 2].try_into().unwrap();
        if le { u16::from_le_bytes(b) } else { u16::from_be_bytes(b) }
    };
    let read_u32 = |off: usize| {
        let b: [u8; 4] = data[off..off + 4].try_into().unwrap();
        if le { u32::from_le_bytes(b) } else { u32::from_be_bytes(b) }
    };
    let read_u64 = |off: usize| {
        let b: [u8; 8] = data[off..off + 8].try_into().unwrap();
        if le { u64::from_le_bytes(b) } else { u64::from_be_bytes(b) }
    };
    let e_type = read_u16(16);
    let e_phoff = read_u64(32) as usize;
    let e_phentsize = read_u16(54) as usize;
    let e_phnum = read_u16(56) as usize;
    if e_phentsize != std::mem::size_of::<Phdr>() && e_phentsize != 56 {
        // tolerate but require at least 56
        if e_phentsize < 56 {
            return Err(LoadError::Parse("bad e_phentsize".into()));
        }
    }
    if e_phoff == 0 || e_phnum == 0 {
        return Err(LoadError::Parse("no program headers".into()));
    }
    if e_phoff + e_phnum * e_phentsize > data.len() {
        return Err(LoadError::Parse("PHDR out of range".into()));
    }
    let mut phdrs = Vec::with_capacity(e_phnum);
    let mut dyn_phdr: Option<Phdr> = None;
    for i in 0..e_phnum {
        let off = e_phoff + i * e_phentsize;
        // Phdr layout: p_type(4) p_flags(4) p_offset(8) p_vaddr(8) p_paddr(8) p_filesz(8) p_memsz(8) p_align(8)
        let p_type = read_u32(off);
        let p_flags = read_u32(off + 4);
        let p_offset = read_u64(off + 8);
        let p_vaddr = read_u64(off + 16);
        let p_filesz = read_u64(off + 32);
        let p_memsz = read_u64(off + 40);
        let p_align = read_u64(off + 48);
        let ph = Phdr { p_type, p_flags, p_offset, p_vaddr, p_filesz, p_memsz, p_align };
        if p_type == elf::program_header::PT_DYNAMIC {
            dyn_phdr = Some(ph.clone());
        }
        phdrs.push(ph);
    }
    Ok((e_type, phdrs, dyn_phdr))
}

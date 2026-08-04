use std::ffi::{c_char, CStr, CString};
use std::path::Path;

/// Read the `DT_NEEDED` dependency list of a shared object from disk.
///
/// Faithful port of `ModLoader::getModDependencies` (`mod_loader.cpp:133`),
/// using `goblin` as the ELF parser. Returns the dependency names in file
/// (dynamic-array) order, mirroring the C++ `ret.emplace_back(...)` loop.
/// Errors mirror the C++ `Log::error` exit conditions as an `Err(String)`:
/// failed read, failed header parse, no `PT_DYNAMIC`.
pub fn get_mod_dependencies(path: &Path) -> Result<Vec<String>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("failed to open mod: {}", e))?;
    let elf = goblin::elf::Elf::parse(&bytes)
        .map_err(|e| format!("failed to parse ELF header: {}", e))?;
    if elf.dynamic.is_none() {
        return Err("couldn't find PT_DYNAMIC".to_string());
    }
    Ok(elf.libraries.into_iter().map(str::to_owned).collect())
}

/// `#[no_mangle]` twin of `ModLoader::getModDependencies`. Returns a
/// null-terminated array of owned C strings, or NULL on error. No caller today
/// (ModLoader is dormant), so the ABI is freeform; the leaked pointers stay
/// valid for the lifetime of the process.
#[no_mangle]
pub unsafe extern "C" fn mc_get_mod_dependencies(path: *const c_char) -> *mut *mut c_char {
    if path.is_null() {
        return std::ptr::null_mut();
    }
    let path = match CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    let deps = match get_mod_dependencies(Path::new(path)) {
        Ok(d) => d,
        Err(e) => {
            log::error!("ModLoader: getModDependencies: {}", e);
            return std::ptr::null_mut();
        }
    };
    let cstrings: Vec<CString> = deps
        .into_iter()
        .map(|s| CString::new(s).expect("no interior NUL"))
        .collect();
    let mut ptrs: Vec<*mut c_char> = cstrings.iter().map(|c| c.as_ptr() as *mut c_char).collect();
    ptrs.push(std::ptr::null_mut());
    // Leak both the C-string buffers and the pointer array so the returned
    // pointers remain valid for the process lifetime.
    std::mem::forget(cstrings);
    Box::into_raw(ptrs.into_boxed_slice()) as *mut *mut c_char
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u16le(v: u16) -> [u8; 2] {
        v.to_le_bytes()
    }
    fn u32le(v: u32) -> [u8; 4] {
        v.to_le_bytes()
    }
    fn u64le(v: u64) -> [u8; 8] {
        v.to_le_bytes()
    }

    /// Build a minimal dynamic x86_64 ELF with a PT_DYNAMIC segment exposing
    /// `liba.so` and `libb.so` as DT_NEEDED.
    fn synth_dep_elf() -> Vec<u8> {
        // Layout: Ehdr(64) + Phdr[PT_LOAD](56) + Phdr[PT_DYNAMIC](56) + Dyn(5*16) + strtab(17)
        const PHDR_OFF: usize = 64;
        const DYN_OFF: usize = PHDR_OFF + 2 * 56;
        const STRTAB_OFF: usize = DYN_OFF + 5 * 16;
        const TOTAL: usize = STRTAB_OFF + 17;
        let mut b = Vec::new();
        // Elf64_Ehdr (64 bytes)
        b.extend_from_slice(&[0x7f, b'E', b'L', b'F', 2, 1, 1, 0]);
        b.extend_from_slice(&[0u8; 8]); // pad
        b.extend_from_slice(&u16le(3)); // e_type = ET_DYN
        b.extend_from_slice(&u16le(62)); // e_machine = EM_X86_64
        b.extend_from_slice(&u32le(1)); // e_version
        b.extend_from_slice(&u64le(0)); // e_entry
        b.extend_from_slice(&u64le(PHDR_OFF as u64)); // e_phoff
        b.extend_from_slice(&u64le(0)); // e_shoff
        b.extend_from_slice(&u32le(0)); // e_flags
        b.extend_from_slice(&u16le(64)); // e_ehsize
        b.extend_from_slice(&u16le(56)); // e_phentsize
        b.extend_from_slice(&u16le(2)); // e_phnum
        b.extend_from_slice(&u16le(0)); // e_shentsize
        b.extend_from_slice(&u16le(0)); // e_shnum
        b.extend_from_slice(&u16le(0)); // e_shstrndx
        assert_eq!(b.len(), PHDR_OFF);
        // PT_LOAD, identity-mapped over the whole file (needed by goblin's
        // vm_to_offset to translate DT_STRTAB's vaddr to a file offset).
        b.extend_from_slice(&u32le(1)); // p_type = PT_LOAD
        b.extend_from_slice(&u32le(4)); // p_flags = PF_R
        b.extend_from_slice(&u64le(0)); // p_offset
        b.extend_from_slice(&u64le(0)); // p_vaddr
        b.extend_from_slice(&u64le(0)); // p_paddr
        b.extend_from_slice(&u64le(TOTAL as u64)); // p_filesz
        b.extend_from_slice(&u64le(TOTAL as u64)); // p_memsz
        b.extend_from_slice(&u64le(8)); // p_align
        // PT_DYNAMIC
        b.extend_from_slice(&u32le(2)); // p_type = PT_DYNAMIC
        b.extend_from_slice(&u32le(4)); // p_flags = PF_R
        b.extend_from_slice(&u64le(DYN_OFF as u64)); // p_offset
        b.extend_from_slice(&u64le(DYN_OFF as u64)); // p_vaddr
        b.extend_from_slice(&u64le(0)); // p_paddr
        b.extend_from_slice(&u64le(5 * 16)); // p_filesz
        b.extend_from_slice(&u64le(5 * 16)); // p_memsz
        b.extend_from_slice(&u64le(8)); // p_align
        assert_eq!(b.len(), DYN_OFF);
        // Dynamic table (5 entries, each 16 bytes)
        let mut push_dyn = |tag: i64, val: u64| {
            b.extend_from_slice(&i64::to_ne_bytes(tag));
            b.extend_from_slice(&u64le(val));
        };
        push_dyn(5, STRTAB_OFF as u64); // DT_STRTAB -> strtab (vaddr == offset)
        push_dyn(10, 17); // DT_STRSZ
        push_dyn(1, 1); // DT_NEEDED -> "liba.so" at strtab offset 1
        push_dyn(1, 9); // DT_NEEDED -> "libb.so" at strtab offset 9
        push_dyn(0, 0); // DT_NULL
        assert_eq!(b.len(), STRTAB_OFF);
        // strtab
        b.push(0);
        b.extend_from_slice(b"liba.so\0");
        b.extend_from_slice(b"libb.so\0");
        assert_eq!(b.len(), TOTAL);
        b
    }

    #[test]
    fn dependencies_in_file_order() {
        let elf = synth_dep_elf();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mod.so");
        std::fs::write(&path, &elf).unwrap();
        assert_eq!(
            get_mod_dependencies(&path).unwrap(),
            vec!["liba.so".to_string(), "libb.so".to_string()]
        );
    }

    #[test]
    fn missing_file_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nope.so");
        assert!(get_mod_dependencies(&path).is_err());
    }

    #[test]
    fn bad_magic_is_error() {
        let mut elf = synth_dep_elf();
        elf[0] = 0x00;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("bad.so");
        std::fs::write(&path, &elf).unwrap();
        assert!(get_mod_dependencies(&path).is_err());
    }

    #[test]
    fn no_pt_dynamic_is_error() {
        let mut elf = synth_dep_elf();
        // Second phdr (PT_DYNAMIC, at offset 64+56): switch p_type 2 -> 0
        elf[64 + 56] = 0;
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("static.so");
        std::fs::write(&path, &elf).unwrap();
        assert!(get_mod_dependencies(&path).is_err());
    }

    #[test]
    fn truncated_header_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("trunc.so");
        std::fs::write(&path, [0x7f, b'E', b'L', b'F']).unwrap();
        assert!(get_mod_dependencies(&path).is_err());
    }
}
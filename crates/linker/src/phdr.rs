use std::fs::File;
use std::os::unix::fs::FileExt;
use std::os::unix::io::AsRawFd;
use std::ptr;

use crate::dlwarning::add_dlwarning;
use crate::mapped_file_fragment::MappedFileFragment;
use crate::sdk_versions::get_application_target_sdk_version;
use crate::utils;
use crate::{dl_err, dl_warn_documented_change};

// kLibraryAlignment from bionic/libc/private/CFIShadow.h
const LIBRARY_ALIGNMENT: usize = 1 << 18;
const LIBC_ALIGNMENT: usize = 4096;

const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];
const EI_CLASS: usize = 4;
const EI_DATA: usize = 5;
const ELFCLASS64: u8 = 2;
const ELFDATA2LSB: u8 = 1;
const EV_CURRENT: u32 = 1;
const ET_DYN: u16 = 3;
const EM_X86_64: u16 = 62;
const EM_AARCH64: u16 = 183;
const EM_ARM: u16 = 40;
const EM_386: u16 = 3;
const EM_MIPS: u16 = 8;

const PT_LOAD: u32 = 1;
const PT_DYNAMIC: u32 = 2;
const PT_INTERP: u32 = 3;
const PT_PHDR: u32 = 6;
const PT_GNU_RELRO: u32 = 0x6474e552;

const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

const SHT_DYNAMIC: u32 = 6;
const SHT_STRTAB: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy)]
struct Elf64_Ehdr {
    e_ident: [u8; 16],
    e_type: u16,
    e_machine: u16,
    e_version: u32,
    e_entry: u64,
    e_phoff: u64,
    e_shoff: u64,
    e_flags: u32,
    e_ehsize: u16,
    e_phentsize: u16,
    e_phnum: u16,
    e_shentsize: u16,
    e_shnum: u16,
    e_shstrndx: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Elf64_Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Elf64_Shdr {
    sh_name: u32,
    sh_type: u32,
    sh_flags: u64,
    sh_addr: u64,
    sh_offset: u64,
    sh_size: u64,
    sh_link: u32,
    sh_info: u32,
    sh_addralign: u64,
    sh_entsize: u64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Elf64_Dyn {
    pub d_tag: i64,
    pub d_val: u64,
}

#[derive(Debug, Default)]
pub struct AddressSpaceParams {
    pub start_addr: *mut libc::c_void,
    pub reserved_size: usize,
    pub must_use_address: bool,
}

fn get_target_elf_machine() -> u16 {
    EM_X86_64
}

fn em_to_string(em: u16) -> &'static str {
    match em {
        EM_386 => "EM_386",
        EM_AARCH64 => "EM_AARCH64",
        EM_ARM => "EM_ARM",
        EM_MIPS => "EM_MIPS",
        EM_X86_64 => "EM_X86_64",
        _ => "EM_???",
    }
}

fn pflags_to_prot(p_flags: u32) -> i32 {
    let mut prot = 0;
    if p_flags & PF_X != 0 {
        prot |= libc::PROT_EXEC;
    }
    if p_flags & PF_R != 0 {
        prot |= libc::PROT_READ;
    }
    if p_flags & PF_W != 0 {
        prot |= libc::PROT_WRITE;
    }
    prot
}

fn page_size() -> i64 {
    unsafe { libc::sysconf(libc::_SC_PAGESIZE) }
}

fn is_page_size_4096() -> bool {
    page_size() == 4096
}

fn page_start(addr: i64) -> i64 {
    let ps = page_size();
    addr & !(ps - 1)
}

fn page_end(addr: i64) -> i64 {
    let ps = page_size();
    let ps_mask = ps - 1;
    (addr + ps_mask) & !ps_mask
}

fn page_offset(addr: i64) -> i64 {
    let ps = page_size();
    addr & (ps - 1)
}

fn align_up(x: usize, align: usize) -> usize {
    (x + align - 1) & !(align - 1)
}

fn align_down(x: usize, align: usize) -> usize {
    x & !(align - 1)
}

pub fn phdr_table_get_load_size(
    phdr_table: &[Elf64_Phdr],
    mut writable_after_exec: Option<&mut Option<u64>>,
) -> (usize, u64, u64) {
    let mut min_vaddr = u64::MAX;
    let mut max_vaddr = 0u64;
    let mut found_pt_load = false;
    let mut prev_exec = false;

    for phdr in phdr_table {
        if phdr.p_type != PT_LOAD {
            continue;
        }
        found_pt_load = true;

        if prev_exec && (phdr.p_flags & PF_W) != 0 {
            if let Some(ref mut wa) = writable_after_exec {
                **wa = Some(phdr.p_vaddr);
            }
        }
        prev_exec = (phdr.p_flags & PF_X) != 0;

        if phdr.p_vaddr < min_vaddr {
            min_vaddr = phdr.p_vaddr;
        }
        if phdr.p_vaddr + phdr.p_memsz > max_vaddr {
            max_vaddr = phdr.p_vaddr + phdr.p_memsz;
        }
    }

    if !found_pt_load {
        min_vaddr = 0;
    }

    min_vaddr = page_start(min_vaddr as i64) as u64;
    max_vaddr = page_end(max_vaddr as i64) as u64;

    (max_vaddr.saturating_sub(min_vaddr) as usize, min_vaddr, max_vaddr)
}

fn reserve_aligned(size: usize, align: usize) -> *mut libc::c_void {
    if !is_page_size_4096() {
        let prot = libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC;
        let mmap_flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;
        let ptr = unsafe {
            libc::mmap(ptr::null_mut(), size, prot, mmap_flags, -1, 0)
        };
        if ptr == libc::MAP_FAILED {
            return ptr::null_mut();
        }
        return ptr;
    }

    let mmap_flags = libc::MAP_PRIVATE | libc::MAP_ANONYMOUS;

    if align == LIBC_ALIGNMENT {
        let ptr = unsafe {
            libc::mmap(ptr::null_mut(), size, libc::PROT_NONE, mmap_flags, -1, 0)
        };
        if ptr == libc::MAP_FAILED {
            return ptr::null_mut();
        }
        return ptr;
    }

    let mmap_size = align_up(size, align) + align - LIBC_ALIGNMENT;
    let mmap_ptr = unsafe {
        libc::mmap(ptr::null_mut(), mmap_size, libc::PROT_NONE, mmap_flags, -1, 0)
    };
    if mmap_ptr == libc::MAP_FAILED {
        return ptr::null_mut();
    }

    let first = align_up(mmap_ptr as usize, align);
    let _last = align_down(mmap_ptr as usize + mmap_size, align) - size;

    let n = 0usize; // No randomization during init
    let start = first + n * LIBC_ALIGNMENT;
    let start_ptr = start as *mut libc::c_void;

    unsafe {
        libc::munmap(mmap_ptr, start - mmap_ptr as usize);
        libc::munmap(
            (start + size) as *mut libc::c_void,
            mmap_ptr as usize + mmap_size - (start + size),
        );
    }

    start_ptr
}

pub fn phdr_table_protect_segments(_phdr_table: &[Elf64_Phdr], _load_bias: u64) -> i32 {
    0
}

pub fn phdr_table_unprotect_segments(_phdr_table: &[Elf64_Phdr], _load_bias: u64) -> i32 {
    0
}

pub fn phdr_table_protect_gnu_relro(_phdr_table: &[Elf64_Phdr], _load_bias: u64) -> i32 {
    0
}

pub fn phdr_table_serialize_gnu_relro(
    phdr_table: &[Elf64_Phdr],
    load_bias: u64,
    fd: &mut File,
    file_offset: &mut usize,
) -> i32 {
    for phdr in phdr_table {
        if phdr.p_type != PT_GNU_RELRO {
            continue;
        }

        let seg_page_start = page_start(phdr.p_vaddr as i64) + load_bias as i64;
        let seg_page_end = page_end((phdr.p_vaddr + phdr.p_memsz) as i64) + load_bias as i64;
        let size = (seg_page_end - seg_page_start) as usize;

        let slice = unsafe {
            std::slice::from_raw_parts(seg_page_start as *const u8, size)
        };

        if let Err(_) = fd.write_at(slice, *file_offset as u64) {
            return -1;
        }

        let map_flags = libc::MAP_PRIVATE | libc::MAP_FIXED;
        let map_ptr = unsafe {
            libc::mmap(
                seg_page_start as *mut libc::c_void,
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                map_flags,
                fd.as_raw_fd(),
                *file_offset as i64,
            )
        };
        if map_ptr == libc::MAP_FAILED {
            return -1;
        }

        *file_offset += size;
    }
    0
}

pub fn phdr_table_map_gnu_relro(
    phdr_table: &[Elf64_Phdr],
    load_bias: u64,
    fd: &File,
    file_offset: &mut usize,
) -> i32 {
    use std::os::unix::io::AsRawFd;

    let metadata = match fd.metadata() {
        Ok(m) => m,
        Err(_) => return -1,
    };
    let file_size = metadata.len() as usize;

    let temp_mapping = if file_size > 0 {
        unsafe {
            libc::mmap(
                ptr::null_mut(),
                file_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE,
                fd.as_raw_fd(),
                0,
            )
        }
    } else {
        ptr::null_mut()
    };

    if file_size > 0 && temp_mapping == libc::MAP_FAILED {
        return -1;
    }

    let mut local_offset = *file_offset;
    for phdr in phdr_table {
        if phdr.p_type != PT_GNU_RELRO {
            continue;
        }

        let seg_page_start = page_start(phdr.p_vaddr as i64) + load_bias as i64;
        let seg_page_end = page_end((phdr.p_vaddr + phdr.p_memsz) as i64) + load_bias as i64;
        let size = (seg_page_end - seg_page_start) as usize;

        if file_size.saturating_sub(local_offset) < size {
            break;
        }

        let file_base = (temp_mapping as *const u8 as usize + local_offset) as *const u8;
        let mem_base = seg_page_start as *const u8;

        let mut match_offset = 0usize;
        while match_offset < size {
            while match_offset < size {
                let remaining = size - match_offset;
                let cmp_size = remaining.min(LIBC_ALIGNMENT);
                unsafe {
                    if libc::memcmp(
                        mem_base.add(match_offset) as *const libc::c_void,
                        file_base.add(match_offset) as *const libc::c_void,
                        cmp_size,
                    ) != 0
                    {
                        match_offset += cmp_size;
                    } else {
                        break;
                    }
                }
            }

            let mut mismatch_offset = match_offset;
            while mismatch_offset < size {
                let remaining = size - mismatch_offset;
                let cmp_size = remaining.min(LIBC_ALIGNMENT);
                unsafe {
                    if libc::memcmp(
                        mem_base.add(mismatch_offset) as *const libc::c_void,
                        file_base.add(mismatch_offset) as *const libc::c_void,
                        cmp_size,
                    ) == 0
                    {
                        mismatch_offset += cmp_size;
                    } else {
                        break;
                    }
                }
            }

            if mismatch_offset > match_offset {
                let map_flags = libc::MAP_PRIVATE | libc::MAP_FIXED;
                let map_ptr = unsafe {
                    libc::mmap(
                        (mem_base as usize + match_offset) as *mut libc::c_void,
                        mismatch_offset - match_offset,
                        libc::PROT_READ | libc::PROT_WRITE,
                        map_flags,
                        fd.as_raw_fd(),
                        (local_offset + match_offset) as i64,
                    )
                };
                if map_ptr == libc::MAP_FAILED {
                    if file_size > 0 {
                        unsafe { libc::munmap(temp_mapping, file_size); }
                    }
                    return -1;
                }
            }

            match_offset = mismatch_offset;
        }

        local_offset += size;
    }

    if file_size > 0 {
        unsafe { libc::munmap(temp_mapping, file_size); }
    }
    *file_offset = local_offset;
    0
}

pub fn phdr_table_get_dynamic_section(
    phdr_table: &[Elf64_Phdr],
    load_bias: u64,
) -> (Option<*const Elf64_Dyn>, Option<u32>) {
    for phdr in phdr_table {
        if phdr.p_type == PT_DYNAMIC {
            let dynamic = (load_bias + phdr.p_vaddr) as *const Elf64_Dyn;
            return (Some(dynamic), Some(phdr.p_flags));
        }
    }
    (None, None)
}

pub fn phdr_table_get_interpreter_name(
    phdr_table: &[Elf64_Phdr],
    load_bias: u64,
) -> Option<*const i8> {
    for phdr in phdr_table {
        if phdr.p_type == PT_INTERP {
            return Some((load_bias + phdr.p_vaddr) as *const i8);
        }
    }
    None
}

pub struct ElfReader {
    did_read: bool,
    did_load: bool,
    name: String,
    fd: Option<File>,
    file_offset: i64,
    file_size: i64,
    header: Option<Elf64_Ehdr>,
    phdr_num: usize,
    phdr_fragment: MappedFileFragment,
    phdr_table: *const Elf64_Phdr,
    shdr_num: usize,
    shdr_fragment: MappedFileFragment,
    shdr_table: *const Elf64_Shdr,
    dynamic_fragment: MappedFileFragment,
    dynamic: *const Elf64_Dyn,
    strtab_fragment: MappedFileFragment,
    strtab: *const u8,
    strtab_size: usize,
    load_start: *mut libc::c_void,
    load_size: usize,
    load_bias: u64,
    loaded_phdr: *const Elf64_Phdr,
    mapped_by_caller: bool,
}

impl ElfReader {
    pub fn new() -> Self {
        ElfReader {
            did_read: false,
            did_load: false,
            name: String::new(),
            fd: None,
            file_offset: 0,
            file_size: 0,
            header: None,
            phdr_num: 0,
            phdr_fragment: MappedFileFragment::new(),
            phdr_table: ptr::null(),
            shdr_num: 0,
            shdr_fragment: MappedFileFragment::new(),
            shdr_table: ptr::null(),
            dynamic_fragment: MappedFileFragment::new(),
            dynamic: ptr::null(),
            strtab_fragment: MappedFileFragment::new(),
            strtab: ptr::null(),
            strtab_size: 0,
            load_start: ptr::null_mut(),
            load_size: 0,
            load_bias: 0,
            loaded_phdr: ptr::null(),
            mapped_by_caller: false,
        }
    }

    pub fn read(&mut self, name: &str, fd: File, file_offset: i64, file_size: i64) -> bool {
        if self.did_read {
            return true;
        }
        self.name = name.to_string();
        self.fd = Some(fd);
        self.file_offset = file_offset;
        self.file_size = file_size;

        if self.read_elf_header() && self.verify_elf_header()
            && self.read_program_headers() && self.read_section_headers()
            && self.read_dynamic_section()
        {
            self.did_read = true;
        }

        self.did_read
    }

    pub fn load(&mut self, address_space: Option<&mut AddressSpaceParams>) -> bool {
        assert!(self.did_read);
        if self.did_load {
            return true;
        }
        if self.reserve_address_space(address_space) && self.load_segments() && self.find_phdr() {
            self.did_load = true;
        }
        self.did_load
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn phdr_count(&self) -> usize {
        self.phdr_num
    }

    pub fn load_start(&self) -> u64 {
        self.load_start as u64
    }

    pub fn load_size(&self) -> usize {
        self.load_size
    }

    pub fn load_bias(&self) -> u64 {
        self.load_bias
    }

    pub fn loaded_phdr(&self) -> *const Elf64_Phdr {
        self.loaded_phdr
    }

    pub fn dynamic(&self) -> *const Elf64_Dyn {
        self.dynamic
    }

    pub fn get_string(&self, index: u32) -> Option<&str> {
        if self.strtab.is_null() {
            return None;
        }
        if index as usize >= self.strtab_size {
            return None;
        }
        let ptr = unsafe { self.strtab.add(index as usize) as *const i8 };
        let cstr = unsafe { std::ffi::CStr::from_ptr(ptr) };
        cstr.to_str().ok()
    }

    pub fn is_mapped_by_caller(&self) -> bool {
        self.mapped_by_caller
    }

    pub fn entry_point(&self) -> Option<u64> {
        self.header.map(|h| h.e_entry + self.load_bias)
    }

    fn read_elf_header(&mut self) -> bool {
        let fd = self.fd.as_ref().unwrap();
        let mut header = Elf64_Ehdr {
            e_ident: [0u8; 16],
            e_type: 0,
            e_machine: 0,
            e_version: 0,
            e_entry: 0,
            e_phoff: 0,
            e_shoff: 0,
            e_flags: 0,
            e_ehsize: 0,
            e_phentsize: 0,
            e_phnum: 0,
            e_shentsize: 0,
            e_shnum: 0,
            e_shstrndx: 0,
        };
        let header_slice = unsafe {
            std::slice::from_raw_parts_mut(
                &mut header as *mut Elf64_Ehdr as *mut u8,
                std::mem::size_of::<Elf64_Ehdr>(),
            )
        };
        match fd.read_exact_at(header_slice, self.file_offset as u64) {
            Ok(_) => {
                self.header = Some(header);
                true
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::UnexpectedEof {
                    dl_err!("\"{}\" is too small to be an ELF executable", self.name);
                } else {
                    dl_err!("can't read file \"{}\": {}", self.name, e);
                }
                false
            }
        }
    }

    fn verify_elf_header(&self) -> bool {
        let header = match self.header {
            Some(h) => h,
            None => return false,
        };

        if header.e_ident[..4] != ELF_MAGIC {
            dl_err!(
                "\"{}\" has bad ELF magic: {:02x}{:02x}{:02x}{:02x}",
                self.name,
                header.e_ident[0], header.e_ident[1], header.e_ident[2], header.e_ident[3]
            );
            return false;
        }

        let elf_class = header.e_ident[EI_CLASS];
        if elf_class != ELFCLASS64 {
            if elf_class == 1 {
                dl_err!("\"{}\" is 32-bit instead of 64-bit", self.name);
            } else {
                dl_err!("\"{}\" has unknown ELF class: {}", self.name, elf_class);
            }
            return false;
        }

        if header.e_ident[EI_DATA] != ELFDATA2LSB {
            dl_err!(
                "\"{}\" not little-endian: {}",
                self.name,
                header.e_ident[EI_DATA]
            );
            return false;
        }

        if header.e_type != ET_DYN {
            dl_err!(
                "\"{}\" has unexpected e_type: {}",
                self.name,
                header.e_type
            );
            return false;
        }

        if header.e_version != EV_CURRENT {
            dl_err!(
                "\"{}\" has unexpected e_version: {}",
                self.name,
                header.e_version
            );
            return false;
        }

        let target_machine = get_target_elf_machine();
        if header.e_machine != target_machine {
            dl_err!(
                "\"{}\" is for {} ({}) instead of {} ({})",
                self.name,
                em_to_string(header.e_machine),
                header.e_machine,
                em_to_string(target_machine),
                target_machine,
            );
            return false;
        }

        let shdr_size = std::mem::size_of::<Elf64_Shdr>() as u16;
        if header.e_shentsize != shdr_size {
            if get_application_target_sdk_version() >= 26 {
                dl_err!(
                    "\"{}\" has unsupported e_shentsize: 0x{:x} (expected 0x{:x})",
                    self.name,
                    header.e_shentsize,
                    shdr_size,
                );
                return false;
            }
            dl_warn_documented_change!(
                26,
                "invalid-elf-header_section-headers-enforced-for-api-level-26",
                "\"{}\" has unsupported e_shentsize 0x{:x} (expected 0x{:x})",
                self.name,
                header.e_shentsize,
                shdr_size,
            );
            add_dlwarning(&self.name, "has invalid ELF header", None);
        }

        if header.e_shstrndx == 0 {
            if get_application_target_sdk_version() >= 26 {
                dl_err!("\"{}\" has invalid e_shstrndx", self.name);
                return false;
            }
            dl_warn_documented_change!(
                26,
                "invalid-elf-header_section-headers-enforced-for-api-level-26",
                "\"{}\" has invalid e_shstrndx",
                self.name,
            );
            add_dlwarning(&self.name, "has invalid ELF header", None);
        }

        true
    }

    fn check_file_range(&self, offset: u64, size: usize, alignment: usize) -> bool {
        let range_start = match utils::safe_add(self.file_offset, offset as usize) {
            Some(s) => s,
            None => return false,
        };
        let range_end = match utils::safe_add(range_start, size) {
            Some(s) => s,
            None => return false,
        };
        offset > 0
            && range_start < self.file_size
            && range_end <= self.file_size
            && offset as usize % alignment == 0
    }

    fn read_program_headers(&mut self) -> bool {
        let header = match self.header {
            Some(h) => h,
            None => return false,
        };

        self.phdr_num = header.e_phnum as usize;

        if self.phdr_num < 1 || self.phdr_num > 65536 / std::mem::size_of::<Elf64_Phdr>() {
            dl_err!("\"{}\" has invalid e_phnum: {}", self.name, self.phdr_num);
            return false;
        }

        let size = self.phdr_num * std::mem::size_of::<Elf64_Phdr>();
        if !self.check_file_range(header.e_phoff, size, std::mem::align_of::<Elf64_Phdr>()) {
            dl_err!(
                "\"{}\" has invalid phdr offset/size: {}/{}",
                self.name,
                header.e_phoff,
                size,
            );
            return false;
        }

        let fd = self.fd.as_ref().unwrap();
        if !self.phdr_fragment.map(
            fd.as_raw_fd(),
            self.file_offset,
            header.e_phoff as usize,
            size,
        ) {
            dl_err!("\"{}\" phdr mmap failed", self.name);
            return false;
        }

        self.phdr_table = self.phdr_fragment.data() as *const Elf64_Phdr;
        true
    }

    fn read_section_headers(&mut self) -> bool {
        let header = match self.header {
            Some(h) => h,
            None => return false,
        };

        self.shdr_num = header.e_shnum as usize;

        if self.shdr_num == 0 {
            dl_err!("\"{}\" has no section headers", self.name);
            return false;
        }

        let size = self.shdr_num * std::mem::size_of::<Elf64_Shdr>();
        if !self.check_file_range(
            header.e_shoff,
            size,
            std::mem::align_of::<Elf64_Shdr>(),
        ) {
            dl_err!(
                "\"{}\" has invalid shdr offset/size: {}/{}",
                self.name,
                header.e_shoff,
                size,
            );
            return false;
        }

        let fd = self.fd.as_ref().unwrap();
        if !self.shdr_fragment.map(
            fd.as_raw_fd(),
            self.file_offset,
            header.e_shoff as usize,
            size,
        ) {
            dl_err!("\"{}\" shdr mmap failed", self.name);
            return false;
        }

        self.shdr_table = self.shdr_fragment.data() as *const Elf64_Shdr;
        true
    }

    fn read_dynamic_section(&mut self) -> bool {
        let shdr_table = self.shdr_table;
        let shdr_num = self.shdr_num;

        if shdr_table.is_null() {
            dl_err!("\"{}\" section headers not loaded", self.name);
            return false;
        }

        let mut dynamic_shdr: Option<Elf64_Shdr> = None;
        for i in 0..shdr_num {
            let shdr = unsafe { &*shdr_table.add(i) };
            if shdr.sh_type == SHT_DYNAMIC {
                dynamic_shdr = Some(*shdr);
                break;
            }
        }

        let dynamic_shdr = match dynamic_shdr {
            Some(s) => s,
            None => {
                dl_err!("\"{}\" .dynamic section header was not found", self.name);
                return false;
            }
        };

        let phdr_table_slice = self.phdr_slice();
        let mut pt_dynamic_offset = 0u64;
        let mut pt_dynamic_filesz = 0u64;
        for phdr in phdr_table_slice {
            if phdr.p_type == PT_DYNAMIC {
                pt_dynamic_offset = phdr.p_offset;
                pt_dynamic_filesz = phdr.p_filesz;
            }
        }

        if pt_dynamic_offset != dynamic_shdr.sh_offset {
            if get_application_target_sdk_version() >= 26 {
                dl_err!(
                    "\"{}\" .dynamic section has invalid offset: 0x{:x}, \
                     expected to match PT_DYNAMIC offset: 0x{:x}",
                    self.name,
                    dynamic_shdr.sh_offset,
                    pt_dynamic_offset,
                );
                return false;
            }
            dl_warn_documented_change!(
                26,
                "invalid-elf-header_section-headers-enforced-for-api-level-26",
                "\"{}\" .dynamic section has invalid offset: 0x{:x} \
                 (expected to match PT_DYNAMIC offset 0x{:x})",
                self.name,
                dynamic_shdr.sh_offset,
                pt_dynamic_offset,
            );
            add_dlwarning(&self.name, "invalid .dynamic section", None);
        }

        if pt_dynamic_filesz != dynamic_shdr.sh_size {
            if get_application_target_sdk_version() >= 26 {
                dl_err!(
                    "\"{}\" .dynamic section has invalid size: 0x{:x}, \
                     expected to match PT_DYNAMIC filesz: 0x{:x}",
                    self.name,
                    dynamic_shdr.sh_size,
                    pt_dynamic_filesz,
                );
                return false;
            }
            dl_warn_documented_change!(
                26,
                "invalid-elf-header_section-headers-enforced-for-api-level-26",
                "\"{}\" .dynamic section has invalid size: 0x{:x} \
                 (expected to match PT_DYNAMIC filesz 0x{:x})",
                self.name,
                dynamic_shdr.sh_size,
                pt_dynamic_filesz,
            );
            add_dlwarning(&self.name, "invalid .dynamic section", None);
        }

        if dynamic_shdr.sh_link >= shdr_num as u32 {
            dl_err!(
                "\"{}\" .dynamic section has invalid sh_link: {}",
                self.name,
                dynamic_shdr.sh_link,
            );
            return false;
        }

        let strtab_shdr = unsafe { &*shdr_table.add(dynamic_shdr.sh_link as usize) };
        if strtab_shdr.sh_type != SHT_STRTAB {
            dl_err!(
                "\"{}\" .dynamic section has invalid link({}) sh_type: {} (expected SHT_STRTAB)",
                self.name,
                dynamic_shdr.sh_link,
                strtab_shdr.sh_type,
            );
            return false;
        }

        if !self.check_file_range(
            dynamic_shdr.sh_offset,
            dynamic_shdr.sh_size as usize,
            std::mem::align_of::<Elf64_Dyn>(),
        ) {
            dl_err!("\"{}\" has invalid offset/size of .dynamic section", self.name);
            return false;
        }

        let fd = self.fd.as_ref().unwrap();
        if !self.dynamic_fragment.map(
            fd.as_raw_fd(),
            self.file_offset,
            dynamic_shdr.sh_offset as usize,
            dynamic_shdr.sh_size as usize,
        ) {
            dl_err!("\"{}\" dynamic section mmap failed", self.name);
            return false;
        }

        self.dynamic = self.dynamic_fragment.data() as *const Elf64_Dyn;

        if !self.check_file_range(
            strtab_shdr.sh_offset,
            strtab_shdr.sh_size as usize,
            std::mem::align_of::<u8>(),
        ) {
            dl_err!(
                "\"{}\" has invalid offset/size of the .strtab section linked from .dynamic section",
                self.name,
            );
            return false;
        }

        if !self.strtab_fragment.map(
            fd.as_raw_fd(),
            self.file_offset,
            strtab_shdr.sh_offset as usize,
            strtab_shdr.sh_size as usize,
        ) {
            dl_err!("\"{}\" strtab section mmap failed", self.name);
            return false;
        }

        self.strtab = self.strtab_fragment.data();
        self.strtab_size = self.strtab_fragment.size();
        true
    }

    fn phdr_slice(&self) -> &[Elf64_Phdr] {
        if self.phdr_table.is_null() || self.phdr_num == 0 {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.phdr_table, self.phdr_num) }
    }

    fn reserve_address_space(&mut self, mut address_space: Option<&mut AddressSpaceParams>) -> bool {
        let phdrs = self.phdr_slice();
        let mut writable_after_exec: Option<u64> = None;
        let (load_size, min_vaddr, _max_vaddr) =
            phdr_table_get_load_size(phdrs, Some(&mut writable_after_exec));

        if load_size == 0 {
            dl_err!("\"{}\" has no loadable segments", self.name);
            return false;
        }

        self.load_size = load_size;

        let start: *mut libc::c_void = match address_space {
            Some(ref mut ap) if load_size <= ap.reserved_size => {
                let s = ap.start_addr;
                self.mapped_by_caller = true;
                ap.start_addr = unsafe { (ap.start_addr as *mut u8).add(load_size) as *mut libc::c_void };
                ap.reserved_size -= load_size;
                s
            }
            _ => {
                let s = reserve_aligned(load_size, LIBRARY_ALIGNMENT);
                if s.is_null() {
                    dl_err!(
                        "couldn't reserve {} bytes of address space for \"{}\"",
                        load_size,
                        self.name,
                    );
                    return false;
                }
                s
            }
        };

        self.load_start = start;
        self.load_bias = (start as u64).wrapping_sub(min_vaddr);
        true
    }

    fn load_segments(&mut self) -> bool {
        let phdrs = self.phdr_slice();

        for phdr in phdrs {
            if phdr.p_type != PT_LOAD {
                continue;
            }

            let seg_start = (phdr.p_vaddr as i64).wrapping_add(self.load_bias as i64);
            let seg_end = seg_start.wrapping_add(phdr.p_memsz as i64);
            let seg_page_start = page_start(seg_start);
            let seg_page_end = page_end(seg_end);
            let seg_file_end = seg_start.wrapping_add(phdr.p_filesz as i64);
            let file_start = phdr.p_offset as i64;
            let file_end = file_start.wrapping_add(phdr.p_filesz as i64);
            let file_page_start = page_start(file_start);
            let file_length = file_end - file_page_start;

            if self.file_size <= 0 {
                dl_err!("\"{}\" invalid file size: {}", self.name, self.file_size);
                return false;
            }

            if file_end > self.file_size {
                dl_err!(
                    "invalid ELF file \"{}\" load segment: p_offset (0x{:x}) + p_filesz (0x{:x}) = 0x{:x} past end of file (0x{:x})",
                    self.name,
                    phdr.p_offset,
                    phdr.p_filesz,
                    file_end as u64,
                    self.file_size as u64,
                );
                return false;
            }

            let prot = pflags_to_prot(phdr.p_flags);

            if !is_page_size_4096() {
                let fd = self.fd.as_ref().unwrap();
                let seg_addr = seg_page_start as *mut libc::c_void;

                if file_length != 0 {
                    let page_offset_val = self.file_offset + file_page_start;
                    if let Err(e) = FileExt::read_exact_at(
                        fd,
                        unsafe { std::slice::from_raw_parts_mut(seg_addr as *mut u8, file_length as usize) },
                        page_offset_val as u64,
                    ) {
                        dl_err!(
                            "couldn't read \"{}\" segment: {}",
                            self.name,
                            e,
                        );
                        return false;
                    }
                }
                continue;
            }

            if file_length != 0 {
                let fd = self.fd.as_ref().unwrap();
                let map_flags = libc::MAP_FIXED | libc::MAP_PRIVATE;
                let seg_addr = unsafe {
                    libc::mmap(
                        seg_page_start as *mut libc::c_void,
                        file_length as usize,
                        prot | libc::PROT_WRITE,
                        map_flags,
                        fd.as_raw_fd(),
                        self.file_offset + file_page_start,
                    )
                };
                if seg_addr == libc::MAP_FAILED {
                    dl_err!(
                        "couldn't map \"{}\" segment: {}",
                        self.name,
                        std::io::Error::last_os_error(),
                    );
                    return false;
                }
            }

            if (phdr.p_flags & PF_W) != 0 && page_offset(seg_file_end) > 0 {
                unsafe {
                    libc::memset(
                        seg_file_end as *mut libc::c_void,
                        0,
                        (page_size() - page_offset(seg_file_end)) as usize,
                    );
                }
            }

            let seg_file_end_page = page_end(seg_file_end);
            if seg_page_end > seg_file_end_page {
                let zero_map_size = (seg_page_end - seg_file_end_page) as usize;
                let zero_map = unsafe {
                    libc::mmap(
                        seg_file_end_page as *mut libc::c_void,
                        zero_map_size,
                        prot | libc::PROT_WRITE,
                        libc::MAP_FIXED | libc::MAP_ANONYMOUS | libc::MAP_PRIVATE,
                        -1,
                        0,
                    )
                };
                if zero_map == libc::MAP_FAILED {
                    dl_err!(
                        "couldn't zero fill \"{}\" gap: {}",
                        self.name,
                        std::io::Error::last_os_error(),
                    );
                    return false;
                }
            }
        }

        true
    }

    fn find_phdr(&mut self) -> bool {
        let phdrs = self.phdr_slice();

        for phdr in phdrs {
            if phdr.p_type == PT_PHDR {
                return self.check_phdr(
                    (self.load_bias as i64).wrapping_add(phdr.p_vaddr as i64) as u64,
                );
            }
        }

        for phdr in phdrs {
            if phdr.p_type == PT_LOAD {
                if phdr.p_offset == 0 {
                    let elf_addr = (self.load_bias as i64).wrapping_add(phdr.p_vaddr as i64) as u64;
                    let loaded_ehdr = unsafe { &*(elf_addr as *const Elf64_Ehdr) };
                    let offset = loaded_ehdr.e_phoff;
                    return self.check_phdr(elf_addr.wrapping_add(offset));
                }
                break;
            }
        }

        dl_err!("can't find loaded phdr for \"{}\"", self.name);
        false
    }

    fn check_phdr(&mut self, loaded: u64) -> bool {
        let phdrs = self.phdr_slice();
        let loaded_end = loaded + (self.phdr_num * std::mem::size_of::<Elf64_Phdr>()) as u64;

        for phdr in phdrs {
            if phdr.p_type != PT_LOAD {
                continue;
            }
            let seg_start = (phdr.p_vaddr as i64).wrapping_add(self.load_bias as i64) as u64;
            let seg_end = seg_start + phdr.p_filesz;
            if seg_start <= loaded && loaded_end <= seg_end {
                self.loaded_phdr = loaded as *const Elf64_Phdr;
                return true;
            }
        }

        dl_err!(
            "\"{}\" loaded phdr {:p} not in loadable segment",
            self.name,
            loaded as *const libc::c_void,
        );
        false
    }
}

impl Drop for ElfReader {
    fn drop(&mut self) {
        if !self.did_load || self.mapped_by_caller {
            return;
        }
        if !self.load_start.is_null() && self.load_size > 0 {
            unsafe {
                libc::munmap(self.load_start, self.load_size);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::os::unix::io::AsRawFd;

    fn write_elf_file(phdrs: &[Elf64_Phdr], dyn_data: Option<&[u64]>) -> (File, u64) {
        let mut file = tempfile::tempfile().unwrap();

        let mut ehdr: Elf64_Ehdr = unsafe { std::mem::zeroed() };
        ehdr.e_ident[0..4].copy_from_slice(&ELF_MAGIC);
        ehdr.e_ident[EI_CLASS] = ELFCLASS64;
        ehdr.e_ident[EI_DATA] = ELFDATA2LSB;
        ehdr.e_type = ET_DYN;
        ehdr.e_machine = EM_X86_64;
        ehdr.e_version = EV_CURRENT;
        ehdr.e_ehsize = std::mem::size_of::<Elf64_Ehdr>() as u16;
        ehdr.e_phentsize = std::mem::size_of::<Elf64_Phdr>() as u16;
        ehdr.e_shnum = 2; // dynamic section (index 0, link=1) + strtab section (index 1)
        ehdr.e_shstrndx = 1;

        ehdr.e_shentsize = std::mem::size_of::<Elf64_Shdr>() as u16;
        ehdr.e_phnum = phdrs.len() as u16;
        ehdr.e_phoff = std::mem::size_of::<Elf64_Ehdr>() as u64;

        let shdr_offset = std::mem::size_of::<Elf64_Ehdr>() + phdrs.len() * std::mem::size_of::<Elf64_Phdr>();
        ehdr.e_shoff = shdr_offset as u64;

        let dynamic_data_offset = shdr_offset + 2 * std::mem::size_of::<Elf64_Shdr>();
        let dyn_data = dyn_data.unwrap_or(&[0u64]);
        let dynamic_data_size = dyn_data.len() * 8;
        let strtab_offset = dynamic_data_offset + dynamic_data_size;

        let mut dynamic_shdr: Elf64_Shdr = unsafe { std::mem::zeroed() };
        dynamic_shdr.sh_type = SHT_DYNAMIC;
        dynamic_shdr.sh_offset = dynamic_data_offset as u64;
        dynamic_shdr.sh_size = dynamic_data_size as u64;
        dynamic_shdr.sh_link = 1; // index of strtab section header
        dynamic_shdr.sh_addralign = 8;

        let mut strtab_shdr: Elf64_Shdr = unsafe { std::mem::zeroed() };
        strtab_shdr.sh_type = SHT_STRTAB;
        strtab_shdr.sh_offset = strtab_offset as u64;
        strtab_shdr.sh_size = 1;
        strtab_shdr.sh_addralign = 1;

        let ehdr_slice = unsafe {
            std::slice::from_raw_parts(&ehdr as *const Elf64_Ehdr as *const u8, std::mem::size_of::<Elf64_Ehdr>())
        };
        file.write_all(ehdr_slice).unwrap();

        let phdr_slice = unsafe {
            std::slice::from_raw_parts(phdrs.as_ptr() as *const u8, phdrs.len() * std::mem::size_of::<Elf64_Phdr>())
        };
        file.write_all(phdr_slice).unwrap();

        let dynamic_shdr_slice = unsafe {
            std::slice::from_raw_parts(&dynamic_shdr as *const Elf64_Shdr as *const u8, std::mem::size_of::<Elf64_Shdr>())
        };
        file.write_all(dynamic_shdr_slice).unwrap();

        let strtab_shdr_slice = unsafe {
            std::slice::from_raw_parts(&strtab_shdr as *const Elf64_Shdr as *const u8, std::mem::size_of::<Elf64_Shdr>())
        };
        file.write_all(strtab_shdr_slice).unwrap();

        let dyn_slice = unsafe {
            std::slice::from_raw_parts(dyn_data.as_ptr() as *const u8, dynamic_data_size)
        };
        file.write_all(dyn_slice).unwrap();

        file.write_all(&[0u8]).unwrap();
        file.flush().unwrap();

        let file_size = file.metadata().unwrap().len() as i64;

        // Now patch the phdrs: update PT_DYNAMIC p_offset/p_vaddr to match layout
        // and PT_LOAD to cover the right range. Also fix e_phnum.
        ehdr.e_phnum = phdrs.len() as u16;
        // Re-write ehdr with correct e_phnum
        file.write_all_at(&ehdr_slice, 0).unwrap();

        // Re-write phdrs with correct offsets
        let mut adjusted_phdrs: Vec<Elf64_Phdr> = phdrs.to_vec();
        for phdr in &mut adjusted_phdrs {
            if phdr.p_type == PT_DYNAMIC {
                phdr.p_offset = dynamic_data_offset as u64;
                phdr.p_vaddr = dynamic_data_offset as u64;
                phdr.p_filesz = dynamic_data_size as u64;
                phdr.p_memsz = dynamic_data_size as u64;
            }
        }
        let adjusted_slice = unsafe {
            std::slice::from_raw_parts(adjusted_phdrs.as_ptr() as *const u8, adjusted_phdrs.len() * std::mem::size_of::<Elf64_Phdr>())
        };
        file.write_all_at(&adjusted_slice, ehdr.e_phoff as u64).unwrap();

        file.flush().unwrap();
        (file, dynamic_data_offset as u64)
    }

    #[test]
    fn test_phdr_table_get_load_size_single() {
        let phdrs = [Elf64_Phdr {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_X,
            p_offset: 0,
            p_vaddr: 0x2000,
            p_paddr: 0,
            p_filesz: 0x1000,
            p_memsz: 0x1000,
            p_align: 0x1000,
        }];
        let (size, min_vaddr, _max_vaddr) = phdr_table_get_load_size(&phdrs, None);
        assert!(size >= 0x1000);
        assert_eq!(min_vaddr, 0x2000);
    }

    #[test]
    fn test_phdr_table_get_load_size_multiple() {
        let phdrs = [
            Elf64_Phdr {
                p_type: PT_LOAD,
                p_flags: PF_R | PF_X,
                p_offset: 0,
                p_vaddr: 0x30000,
                p_paddr: 0,
                p_filesz: 0x4000,
                p_memsz: 0x4000,
                p_align: 0x1000,
            },
            Elf64_Phdr {
                p_type: PT_LOAD,
                p_flags: PF_R | PF_W,
                p_offset: 0x4000,
                p_vaddr: 0x40000,
                p_paddr: 0,
                p_filesz: 0x2000,
                p_memsz: 0x8000,
                p_align: 0x1000,
            },
        ];
        let (size, min_vaddr, max_vaddr) = phdr_table_get_load_size(&phdrs, None);
        assert_eq!(min_vaddr, 0x30000);
        assert!(max_vaddr >= 0x48000);
        assert!(size >= 0x18000);
    }

    #[test]
    fn test_phdr_table_get_load_size_no_pt_load() {
        let phdrs = [Elf64_Phdr {
            p_type: 0, // PT_NULL
            p_flags: 0,
            p_offset: 0,
            p_vaddr: 0,
            p_paddr: 0,
            p_filesz: 0,
            p_memsz: 0,
            p_align: 0,
        }];
        let (size, min_vaddr, _) = phdr_table_get_load_size(&phdrs, None);
        assert_eq!(size, 0);
        assert_eq!(min_vaddr, 0);
    }

    #[test]
    fn test_phdr_table_get_load_size_writable_after_exec() {
        let phdrs = [
            Elf64_Phdr {
                p_type: PT_LOAD,
                p_flags: PF_R | PF_X,
                p_offset: 0,
                p_vaddr: 0x1000,
                p_paddr: 0,
                p_filesz: 0x1000,
                p_memsz: 0x1000,
                p_align: 0x1000,
            },
            Elf64_Phdr {
                p_type: PT_LOAD,
                p_flags: PF_R | PF_W,
                p_offset: 0x2000,
                p_vaddr: 0x2000,
                p_paddr: 0,
                p_filesz: 0x1000,
                p_memsz: 0x1000,
                p_align: 0x1000,
            },
        ];
        let mut writable_after_exec: Option<u64> = None;
        phdr_table_get_load_size(&phdrs, Some(&mut writable_after_exec));
        assert_eq!(writable_after_exec, Some(0x2000));
    }

    #[test]
    fn test_elfreader_read_invalid_elf() {
        let mut file = tempfile::tempfile().unwrap();
        file.write_all(b"not an elf file").unwrap();
        file.flush().unwrap();

        let mut reader = ElfReader::new();
        assert!(!reader.read("test.so", file, 0, 15));
    }

    #[test]
    fn test_elfreader_read_valid_minimal() {
        let phdrs = [
            Elf64_Phdr {
                p_type: PT_LOAD,
                p_flags: PF_R | PF_X,
                p_offset: 0x1000,
                p_vaddr: 0x2000,
                p_paddr: 0,
                p_filesz: 0x100,
                p_memsz: 0x100,
                p_align: 0x1000,
            },
            Elf64_Phdr {
                p_type: PT_DYNAMIC,
                p_flags: PF_R | PF_W,
                p_offset: 0,
                p_vaddr: 0,
                p_paddr: 0,
                p_filesz: 0x10,
                p_memsz: 0x10,
                p_align: 0x8,
            },
        ];

        let (file, _dynamic_offset) = write_elf_file(&phdrs, None);
        let file_size = file.metadata().unwrap().len() as i64;

        let mut reader = ElfReader::new();
        assert!(reader.read("test.so", file, 0, file_size));
        assert_eq!(reader.phdr_count(), 2);
    }

    #[test]
    fn test_elfreader_read_with_dynamic() {
        let phdrs = [
            Elf64_Phdr {
                p_type: PT_LOAD,
                p_flags: PF_R | PF_X,
                p_offset: 0x1000,
                p_vaddr: 0x2000,
                p_paddr: 0,
                p_filesz: 0x100,
                p_memsz: 0x100,
                p_align: 0x1000,
            },
            Elf64_Phdr {
                p_type: PT_DYNAMIC,
                p_flags: PF_R | PF_W,
                p_offset: 0,
                p_vaddr: 0,
                p_paddr: 0,
                p_filesz: 0x10,
                p_memsz: 0x10,
                p_align: 0x8,
            },
        ];

        let dyn_data = [0u64]; // DT_NULL

        let (file, _dynamic_offset) = write_elf_file(&phdrs, Some(&dyn_data));
        let file_size = file.metadata().unwrap().len() as i64;
        let file_clone = file.try_clone().unwrap();

        let mut reader = ElfReader::new();
        assert!(reader.read("test.so", file_clone, 0, file_size));
        assert_eq!(reader.phdr_count(), 2);
        assert!(!reader.dynamic().is_null());
    }

    #[test]
    fn test_phdr_table_protect_segments_noop() {
        let phdrs = [Elf64_Phdr {
            p_type: PT_LOAD,
            p_flags: PF_R | PF_X,
            p_offset: 0,
            p_vaddr: 0,
            p_paddr: 0,
            p_filesz: 0,
            p_memsz: 0,
            p_align: 0,
        }];
        assert_eq!(phdr_table_protect_segments(&phdrs, 0), 0);
        assert_eq!(phdr_table_unprotect_segments(&phdrs, 0), 0);
        assert_eq!(phdr_table_protect_gnu_relro(&phdrs, 0), 0);
    }

    #[test]
    fn test_phdr_table_get_dynamic_section_found() {
        let phdrs = [Elf64_Phdr {
            p_type: PT_DYNAMIC,
            p_flags: PF_R | PF_W,
            p_offset: 0,
            p_vaddr: 0x1000,
            p_paddr: 0,
            p_filesz: 0x100,
            p_memsz: 0x100,
            p_align: 0x8,
        }];
        let (dynamic, flags) = phdr_table_get_dynamic_section(&phdrs, 0x10000);
        assert!(dynamic.is_some());
        assert_eq!(flags, Some(PF_R | PF_W));
        assert_eq!(dynamic.unwrap() as u64, 0x11000);
    }

    #[test]
    fn test_phdr_table_get_dynamic_section_not_found() {
        let phdrs = [Elf64_Phdr {
            p_type: PT_LOAD,
            p_flags: PF_R,
            p_offset: 0,
            p_vaddr: 0,
            p_paddr: 0,
            p_filesz: 0,
            p_memsz: 0,
            p_align: 0,
        }];
        let (dynamic, flags) = phdr_table_get_dynamic_section(&phdrs, 0);
        assert!(dynamic.is_none());
        assert!(flags.is_none());
    }

    #[test]
    fn test_phdr_table_get_interpreter_name_found() {
        let phdrs = [Elf64_Phdr {
            p_type: PT_INTERP,
            p_flags: PF_R,
            p_offset: 0,
            p_vaddr: 0x2000,
            p_paddr: 0,
            p_filesz: 0x10,
            p_memsz: 0x10,
            p_align: 0x1,
        }];
        let interp = phdr_table_get_interpreter_name(&phdrs, 0x10000);
        assert!(interp.is_some());
        assert_eq!(interp.unwrap() as u64, 0x12000);
    }

    #[test]
    fn test_phdr_table_get_interpreter_name_not_found() {
        let phdrs = [Elf64_Phdr {
            p_type: PT_LOAD,
            p_flags: PF_R,
            p_offset: 0,
            p_vaddr: 0,
            p_paddr: 0,
            p_filesz: 0,
            p_memsz: 0,
            p_align: 0,
        }];
        assert!(phdr_table_get_interpreter_name(&phdrs, 0).is_none());
    }

    #[test]
    fn test_reserve_aligned_basic() {
        let ptr = reserve_aligned(4096, LIBC_ALIGNMENT);
        assert!(!ptr.is_null());
        unsafe { libc::munmap(ptr, 4096); }
    }

    #[test]
    fn test_reserve_aligned_large_alignment() {
        let ptr = reserve_aligned(4096, LIBRARY_ALIGNMENT);
        assert!(!ptr.is_null());
        assert_eq!((ptr as usize) & (LIBRARY_ALIGNMENT - 1), 0);
        unsafe { libc::munmap(ptr, 4096); }
    }

    #[test]
    fn test_pflags_to_prot() {
        assert_eq!(pflags_to_prot(PF_R), libc::PROT_READ);
        assert_eq!(pflags_to_prot(PF_W), libc::PROT_WRITE);
        assert_eq!(pflags_to_prot(PF_X), libc::PROT_EXEC);
        assert_eq!(
            pflags_to_prot(PF_R | PF_W),
            libc::PROT_READ | libc::PROT_WRITE
        );
        assert_eq!(
            pflags_to_prot(PF_R | PF_X),
            libc::PROT_READ | libc::PROT_EXEC
        );
        assert_eq!(
            pflags_to_prot(PF_R | PF_W | PF_X),
            libc::PROT_READ | libc::PROT_WRITE | libc::PROT_EXEC
        );
        assert_eq!(pflags_to_prot(0), 0);
    }

    #[test]
    fn test_elfreader_load_and_unload() {
        // Create a larger file with space for the PT_LOAD segment
        let file_size = 0x2000i64;
        let mut file = tempfile::tempfile().unwrap();
        // Write a valid ELF header + phdrs at start, then zero pad to file_size
        {
            let phdrs_raw = [
                Elf64_Phdr {
                    p_type: PT_LOAD,
                    p_flags: PF_R | PF_X,
                    p_offset: 0,
                    p_vaddr: 0,
                    p_paddr: 0,
                    p_filesz: 0x100,
                    p_memsz: 0x100,
                    p_align: 0x1000,
                },
                Elf64_Phdr {
                    p_type: PT_DYNAMIC,
                    p_flags: PF_R | PF_W,
                    p_offset: 0x120,
                    p_vaddr: 0x120,
                    p_paddr: 0,
                    p_filesz: 0x8,
                    p_memsz: 0x8,
                    p_align: 0x8,
                },
            ];

            let mut ehdr: Elf64_Ehdr = unsafe { std::mem::zeroed() };
            ehdr.e_ident[0..4].copy_from_slice(&ELF_MAGIC);
            ehdr.e_ident[EI_CLASS] = ELFCLASS64;
            ehdr.e_ident[EI_DATA] = ELFDATA2LSB;
            ehdr.e_type = ET_DYN;
            ehdr.e_machine = EM_X86_64;
            ehdr.e_version = EV_CURRENT;
            ehdr.e_ehsize = std::mem::size_of::<Elf64_Ehdr>() as u16;
            ehdr.e_phentsize = std::mem::size_of::<Elf64_Phdr>() as u16;
            ehdr.e_phnum = phdrs_raw.len() as u16;
            ehdr.e_shentsize = std::mem::size_of::<Elf64_Shdr>() as u16;
            ehdr.e_shnum = 2;
            ehdr.e_shstrndx = 1;
            ehdr.e_phoff = std::mem::size_of::<Elf64_Ehdr>() as u64;

            // Layout: Ehdr + Phdrs + 2 x Shdr + dynamic data (8 bytes) + strtab (1 byte)
            let ehdr_size = std::mem::size_of::<Elf64_Ehdr>();
            let phdrs_size = phdrs_raw.len() * std::mem::size_of::<Elf64_Phdr>();
            let shdr_size = 2 * std::mem::size_of::<Elf64_Shdr>();
            let dynamic_data_size = 8usize; // DT_NULL
            let strtab_size = 1usize;

            ehdr.e_shoff = (ehdr_size + phdrs_size) as u64;

            let dynamic_data_offset = ehdr_size + phdrs_size + shdr_size;

            let mut dynamic_shdr: Elf64_Shdr = unsafe { std::mem::zeroed() };
            dynamic_shdr.sh_type = SHT_DYNAMIC;
            dynamic_shdr.sh_offset = dynamic_data_offset as u64;
            dynamic_shdr.sh_size = dynamic_data_size as u64;
            dynamic_shdr.sh_link = 1;
            dynamic_shdr.sh_addralign = 8;

            let mut strtab_shdr: Elf64_Shdr = unsafe { std::mem::zeroed() };
            strtab_shdr.sh_type = SHT_STRTAB;
            strtab_shdr.sh_offset = (dynamic_data_offset + dynamic_data_size) as u64;
            strtab_shdr.sh_size = strtab_size as u64;
            strtab_shdr.sh_addralign = 1;

            // Write Ehdr
            let ehdr_bytes = unsafe { std::slice::from_raw_parts(&ehdr as *const _ as *const u8, ehdr_size) };
            file.write_all(ehdr_bytes).unwrap();

            // Write Phdrs
            let phdr_bytes = unsafe { std::slice::from_raw_parts(phdrs_raw.as_ptr() as *const u8, phdrs_size) };
            file.write_all(phdr_bytes).unwrap();

            // Write Shdrs
            let shdr_bytes1 = unsafe { std::slice::from_raw_parts(&dynamic_shdr as *const _ as *const u8, std::mem::size_of::<Elf64_Shdr>()) };
            file.write_all(shdr_bytes1).unwrap();
            let shdr_bytes2 = unsafe { std::slice::from_raw_parts(&strtab_shdr as *const _ as *const u8, std::mem::size_of::<Elf64_Shdr>()) };
            file.write_all(shdr_bytes2).unwrap();

            // Write dynamic data
            file.write_all(&[0u8; 8]).unwrap();
            // Write strtab
            file.write_all(&[0u8; 1]).unwrap();

            // Now fix PT_DYNAMIC in the written phdrs
            let mut adjusted = phdrs_raw.to_vec();
            adjusted[1].p_offset = dynamic_data_offset as u64;
            adjusted[1].p_vaddr = dynamic_data_offset as u64;
            let adjusted_bytes = unsafe { std::slice::from_raw_parts(adjusted.as_ptr() as *const u8, phdrs_size) };
            file.write_all_at(adjusted_bytes, ehdr.e_phoff).unwrap();
        }

        // Zero-pad to file_size
        let cur_len = file.metadata().unwrap().len();
        if cur_len < file_size as u64 {
            file.write_all_at(&vec![0u8; (file_size as u64 - cur_len) as usize], cur_len).unwrap();
        }
        file.flush().unwrap();

        let mut reader = ElfReader::new();
        assert!(reader.read("test.so", file, 0, file_size));
        assert!(reader.load(None));
        assert!(reader.load_start() != 0);
        assert!(reader.load_size() > 0);
    }
}

use crate::soinfo::SoInfo;
use crate::symbol;

const K_SHADOW_GRANULARITY: usize = 18;
const K_CFI_CHECK_GRANULARITY: usize = 12;
const K_SHADOW_ALIGN: usize = 1 << K_SHADOW_GRANULARITY;
pub const K_CFI_CHECK_ALIGN: usize = 1 << K_CFI_CHECK_GRANULARITY;
const K_MAX_TARGET_ADDR: usize = 0xffffffffffff;
pub const PAGE_SIZE: usize = 4096;

pub const fn align_up(x: usize, align: usize) -> usize {
    (x + align - 1) & !(align - 1)
}

pub const K_SHADOW_SIZE: usize =
    align_up(K_MAX_TARGET_ADDR >> (K_SHADOW_GRANULARITY - 1), PAGE_SIZE);

pub const fn mem_to_shadow_offset(x: usize) -> usize {
    (x >> K_SHADOW_GRANULARITY) << 1
}

pub const INVALID_SHADOW: u16 = 0;
pub const UNCHECKED_SHADOW: u16 = 1;
pub const REGULAR_SHADOW_MIN: u16 = 2;

struct ShadowWrite {
    dst: *mut u16,
    len: usize,
    tmp: Vec<u16>,
}

impl ShadowWrite {
    fn new(shadow_start: *mut u16, shadow_end: *mut u16) -> Self {
        let len = if shadow_start.is_null() || shadow_end <= shadow_start {
            0
        } else {
            (shadow_end as usize - shadow_start as usize) / 2
        };
        let tmp = if len > 0 {
            unsafe { std::slice::from_raw_parts(shadow_start, len) }.to_vec()
        } else {
            Vec::new()
        };
        Self { dst: shadow_start, len, tmp }
    }

    fn slice_mut(&mut self) -> &mut [u16] {
        let len = self.len;
        unsafe { std::slice::from_raw_parts_mut(self.tmp.as_mut_ptr(), len) }
    }
}

impl Drop for ShadowWrite {
    fn drop(&mut self) {
        if self.len > 0 && !self.dst.is_null() {
            unsafe {
                std::ptr::copy_nonoverlapping(self.tmp.as_ptr(), self.dst, self.len);
            }
        }
    }
}

pub struct CFIShadowWriter {
    shadow_base: Option<usize>,
    pub initial_link_done: bool,
}

impl CFIShadowWriter {
    pub const fn new() -> Self {
        Self { shadow_base: None, initial_link_done: false }
    }

    fn mem_to_shadow(&self, x: usize) -> Option<*mut u16> {
        self.shadow_base
            .map(|base| (base + mem_to_shadow_offset(x)) as *mut u16)
    }

    pub fn fixup_vma_name(&self) {}

    pub fn add_constant(&mut self, begin: usize, end: usize, v: u16) {
        if end <= begin {
            return;
        }
        let shadow_begin = match self.mem_to_shadow(begin) {
            Some(p) => p,
            None => return,
        };
        let shadow_end = match self.mem_to_shadow(end - 1) {
            Some(p) => unsafe { p.add(1) },
            None => return,
        };
        let mut sw = ShadowWrite::new(shadow_begin, shadow_end);
        for slot in sw.slice_mut().iter_mut() {
            *slot = v;
        }
    }

    pub fn add_unchecked(&mut self, begin: usize, end: usize) {
        self.add_constant(begin, end, UNCHECKED_SHADOW);
    }

    pub fn add_invalid(&mut self, begin: usize, end: usize) {
        self.add_constant(begin, end, INVALID_SHADOW);
    }

    pub fn add(&mut self, begin: usize, end: usize, cfi_check: usize) {
        assert_eq!(cfi_check & (K_CFI_CHECK_ALIGN - 1), 0);
        if end <= begin {
            return;
        }
        let begin = std::cmp::max(begin, cfi_check) & !(K_SHADOW_ALIGN - 1);
        if end <= begin {
            return;
        }
        let shadow_begin = match self.mem_to_shadow(begin) {
            Some(p) => p,
            None => return,
        };
        let shadow_end = match self.mem_to_shadow(end - 1) {
            Some(p) => unsafe { p.add(1) },
            None => return,
        };
        let sv_begin = ((begin + K_SHADOW_ALIGN - cfi_check) >> K_CFI_CHECK_GRANULARITY)
            + REGULAR_SHADOW_MIN as usize;
        let sv_step = 1usize << (K_SHADOW_GRANULARITY - K_CFI_CHECK_GRANULARITY);
        let mut sw = ShadowWrite::new(shadow_begin, shadow_end);
        if sw.len == 0 {
            return;
        }
        let mut sv = sv_begin as u16;
        for slot in sw.slice_mut().iter_mut() {
            if sv < sv_begin as u16 {
                *slot = UNCHECKED_SHADOW;
            } else {
                *slot = if *slot == INVALID_SHADOW { sv } else { UNCHECKED_SHADOW };
            }
            sv = sv.wrapping_add(sv_step as u16);
        }
    }

    pub fn map_shadow() -> Option<usize> {
        let p = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                K_SHADOW_SIZE,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
                -1,
                0,
            )
        };
        if p == libc::MAP_FAILED {
            None
        } else {
            Some(p as usize)
        }
    }

    pub fn init_shadow(&mut self, base: usize) {
        self.shadow_base = Some(base);
    }

    pub fn add_library(&mut self, si: &SoInfo) -> bool {
        let Some(_) = self.shadow_base else { return false };
        if si.base == 0 || si.size == 0 {
            return true;
        }
        let cfi_check = Self::find_cfi_check(si);
        if cfi_check == 0 {
            log::info!("[ CFI add {:#x} + {:#x} {} ]", si.base, si.size, si.soname);
            self.add_unchecked(si.base, si.base + si.size);
            return true;
        }
        log::info!("[ CFI add {:#x} + {:#x} {}: {:#x} ]", si.base, si.size, si.soname, cfi_check);
        if cfi_check & (K_CFI_CHECK_ALIGN - 1) != 0 {
            log::error!("unaligned __cfi_check in the library \"{}\"", si.soname);
            return false;
        }
        self.add(si.base, si.base + si.size, cfi_check);
        true
    }

    fn find_cfi_check(si: &SoInfo) -> usize {
        if let Some(&addr) = si.external_symbols.get("__cfi_check") {
            return addr;
        }
        if !si.is_stub {
            if let Some((addr, _)) = symbol::find_symbol(si, "__cfi_check") {
                return addr;
            }
        }
        0
    }

    pub fn after_load(&mut self, si: &SoInfo, solist: &[&SoInfo]) -> bool {
        if !self.initial_link_done {
            return true;
        }
        if self.shadow_base.is_none() {
            return self.maybe_init(Some(si), solist);
        }
        if !self.add_library(si) {
            return false;
        }
        self.fixup_vma_name();
        true
    }

    pub fn before_unload(&mut self, si: &SoInfo) {
        if self.shadow_base.is_none() {
            return;
        }
        if si.base == 0 || si.size == 0 {
            return;
        }
        log::info!("[ CFI remove {:#x} + {:#x}: {} ]", si.base, si.size, si.soname);
        self.add_invalid(si.base, si.base + si.size);
        self.fixup_vma_name();
    }

    pub fn initial_link_done(&mut self, solist: &[&SoInfo]) -> bool {
        assert!(!self.initial_link_done);
        self.initial_link_done = true;
        self.maybe_init(None, solist)
    }

    fn maybe_init(&mut self, new_si: Option<&SoInfo>, solist: &[&SoInfo]) -> bool {
        let found = match new_si {
            Some(si) => Self::find_cfi_check(si) != 0,
            None => solist.iter().any(|si| Self::find_cfi_check(si) != 0),
        };
        if !found {
            return true;
        }
        let base = match Self::map_shadow() {
            Some(b) => b,
            None => return false,
        };
        self.init_shadow(base);
        for si in solist {
            if !self.add_library(si) {
                return false;
            }
        }
        self.fixup_vma_name();
        true
    }

    pub fn cfi_fail(
        call_site_type_id: u64,
        ptr: *mut std::ffi::c_void,
        diag_data: *mut std::ffi::c_void,
        caller_pc: *const std::ffi::c_void,
    ) {
        let handle = match crate::dladdr(caller_pc) {
            Some((h, _)) => h,
            None => unsafe { libc::abort() },
        };
        let cfi_check = match crate::dlsym(handle, "__cfi_check") {
            Some(p) => p as usize,
            None => unsafe { libc::abort() },
        };
        type CfiCheckFn = unsafe extern "C" fn(u64, *mut std::ffi::c_void, *mut std::ffi::c_void);
        let f: CfiCheckFn = unsafe { std::mem::transmute(cfi_check) };
        unsafe { f(call_site_type_id, ptr, diag_data) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mem_to_shadow_offset() {
        assert_eq!(mem_to_shadow_offset(0), 0);
        assert_eq!(mem_to_shadow_offset(1), 0);
        assert_eq!(mem_to_shadow_offset(1 << K_SHADOW_GRANULARITY), 2);
        assert_eq!(mem_to_shadow_offset(1 << (K_SHADOW_GRANULARITY + 1)), 4);
        assert_eq!(mem_to_shadow_offset(0x7f0000000000 as usize), 0x3f800000 as usize);
    }

    #[test]
    fn test_align_up() {
        assert_eq!(align_up(0, 4096), 0);
        assert_eq!(align_up(1, 4096), 4096);
        assert_eq!(align_up(4096, 4096), 4096);
        assert_eq!(align_up(4097, 4096), 8192);
    }

    #[test]
    fn test_shadow_size() {
        let expected = align_up(0xffffffffffffusize >> 17, 4096);
        assert_eq!(K_SHADOW_SIZE, expected);
    }

    #[test]
    fn test_add_constant() {
        let mut shadow = vec![0xAAAAu16; 256];
        let mut writer = CFIShadowWriter::new();
        writer.shadow_base = Some(shadow.as_mut_ptr() as usize);

        writer.add_constant(0, K_SHADOW_ALIGN, INVALID_SHADOW);
        assert_eq!(shadow[0], INVALID_SHADOW);
        assert_eq!(shadow[1], 0xAAAA);
    }

    #[test]
    fn test_add_unchecked() {
        let mut shadow = vec![INVALID_SHADOW; 256];
        let mut writer = CFIShadowWriter::new();
        writer.shadow_base = Some(shadow.as_mut_ptr() as usize);

        writer.add_unchecked(0, K_SHADOW_ALIGN);
        assert_eq!(shadow[0], UNCHECKED_SHADOW);
    }

    #[test]
    fn test_add_invalid() {
        let mut shadow = vec![UNCHECKED_SHADOW; 256];
        let mut writer = CFIShadowWriter::new();
        writer.shadow_base = Some(shadow.as_mut_ptr() as usize);

        writer.add_invalid(0, K_SHADOW_ALIGN);
        assert_eq!(shadow[0], INVALID_SHADOW);
    }

    #[test]
    fn test_add_aligned_cfi_check() {
        let mut shadow = vec![INVALID_SHADOW; 256];
        let mut writer = CFIShadowWriter::new();
        writer.shadow_base = Some(shadow.as_mut_ptr() as usize);

        let cfi_check = K_CFI_CHECK_ALIGN;
        writer.add(0, K_SHADOW_ALIGN * 2, cfi_check);

        let sv_begin = ((0 + K_SHADOW_ALIGN - cfi_check) >> K_CFI_CHECK_GRANULARITY) + REGULAR_SHADOW_MIN as usize;
        assert_eq!(shadow[0], sv_begin as u16);
    }

    #[test]
    fn test_empty_range_noop() {
        let mut shadow = vec![0u16; 256];
        let mut writer = CFIShadowWriter::new();
        writer.shadow_base = Some(shadow.as_mut_ptr() as usize);

        writer.add_constant(10, 10, INVALID_SHADOW);
        writer.add_constant(20, 15, INVALID_SHADOW);
        for &v in &shadow {
            assert_eq!(v, 0);
        }
    }

    #[test]
    fn test_no_shadow_base() {
        let mut writer = CFIShadowWriter::new();
        writer.add_constant(0, 4096, INVALID_SHADOW);
    }

    #[test]
    fn test_add_library_stub() {
        let mut shadow = vec![INVALID_SHADOW; 256];
        let mut writer = CFIShadowWriter::new();
        writer.shadow_base = Some(shadow.as_mut_ptr() as usize);

        let si = SoInfo {
            name: "test.so".into(),
            soname: "test.so".into(),
            base: 0x1000,
            size: 0x4000,
            is_stub: true,
            ..Default::default()
        };

        let result = writer.add_library(&si);
        assert!(result);
        assert_eq!(shadow[0], UNCHECKED_SHADOW);
    }

    #[test]
    fn test_add_library_with_cfi_check() {
        let mut shadow = vec![INVALID_SHADOW; 256];
        let mut writer = CFIShadowWriter::new();
        writer.shadow_base = Some(shadow.as_mut_ptr() as usize);

        let mut si = SoInfo {
            name: "cfi.so".into(),
            soname: "cfi.so".into(),
            base: 0,
            size: 0x10000,
            is_stub: false,
            ..Default::default()
        };
        si.external_symbols.insert("__cfi_check".into(), K_CFI_CHECK_ALIGN);

        let result = writer.add_library(&si);
        assert!(result);
    }

    #[test]
    fn test_add_library_zero_base() {
        let mut shadow = vec![INVALID_SHADOW; 256];
        let mut writer = CFIShadowWriter::new();
        writer.shadow_base = Some(shadow.as_mut_ptr() as usize);

        let si = SoInfo {
            name: "test.so".into(),
            soname: "test.so".into(),
            base: 0,
            size: 0,
            is_stub: true,
            ..Default::default()
        };

        let result = writer.add_library(&si);
        assert!(result);
    }

    #[test]
    fn test_add_library_no_shadow() {
        let mut writer = CFIShadowWriter::new();
        let si = SoInfo {
            name: "test.so".into(),
            soname: "test.so".into(),
            base: 0x1000,
            size: 0x4000,
            ..Default::default()
        };
        let result = writer.add_library(&si);
        assert!(!result);
    }

    #[test]
    fn test_before_unload() {
        let mut shadow = vec![UNCHECKED_SHADOW; 256];
        let mut writer = CFIShadowWriter::new();
        writer.shadow_base = Some(shadow.as_mut_ptr() as usize);

        let si = SoInfo {
            name: "test.so".into(),
            soname: "test.so".into(),
            base: 0x1000,
            size: 0x4000,
            ..Default::default()
        };
        writer.before_unload(&si);
        assert_eq!(shadow[0], INVALID_SHADOW);
    }
}

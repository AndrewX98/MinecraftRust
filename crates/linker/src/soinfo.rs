use std::collections::HashMap;

use crate::Handle;

// Flag constants matching bionic linker_soinfo.h
pub const FLAG_LINKED: u32 = 0x0000_0001;
pub const FLAG_EXE: u32 = 0x0000_0004;
pub const FLAG_LINKER: u32 = 0x0000_0010;
pub const FLAG_GNU_HASH: u32 = 0x0000_0040;
pub const FLAG_MAPPED_BY_CALLER: u32 = 0x0000_0080;
pub const FLAG_IMAGE_LINKED: u32 = 0x0000_0100;
pub const FLAG_RESERVED: u32 = 0x0000_0200;
pub const FLAG_PRELINKED: u32 = 0x0000_0400;
pub const FLAG_NEW_SOINFO: u32 = 0x4000_0000;

#[derive(Clone, Default, Debug)]
pub struct SoInfo {
    pub name: String,
    pub soname: String,
    pub base: usize,
    pub load_bias: usize,
    pub size: usize,
    pub dynamic: Option<usize>,
    pub symtab: Option<usize>,
    pub symtab_size: usize,
    pub strtab: Option<usize>,
    pub strtab_size: usize,
    pub gnu_hash: Option<usize>,
    pub sysv_hash: Option<usize>,
    pub bucket_count: usize,
    pub bucket: Vec<u32>,
    pub chain: Vec<u32>,
    pub gnu_bucket: Vec<u32>,
    pub gnu_chain: Vec<u32>,
    pub gnu_bloom_filter: Vec<usize>,
    pub gnu_bloom_shift: usize,
    pub gnu_bloom_n: usize,
    pub pltrel: Option<(usize, usize)>,
    pub pltrel_type: RelocType,
    pub rel: Option<(usize, usize)>,
    pub rela: Option<(usize, usize)>,
    pub rel_size: usize,
    pub init: Option<usize>,
    pub init_func: Option<unsafe extern "C" fn(i32, *mut *mut i8, *mut *mut i8)>,
    pub init_array: Option<(usize, usize)>,
    pub fini: Option<usize>,
    pub fini_func: Option<unsafe extern "C" fn()>,
    pub fini_array: Option<(usize, usize)>,
    pub preinit_array: Option<(usize, usize)>,
    pub dependencies: Vec<String>,
    pub external_symbols: HashMap<String, usize>,
    pub is_stub: bool,
    pub tls_segment: Option<TlsSegment>,
    pub tls_module_id: usize,
    pub pt_gnu_relro: Option<(usize, usize)>,
    pub dt_flags_1: u64,
    pub rtld_flags: i32,
    pub primary_namespace: Option<String>,
    pub secondary_namespaces: Vec<String>,
    pub parents: Vec<Handle>,
    pub children: Vec<Handle>,
    pub flags: u32,
    pub constructors_called: bool,
    pub handle: Handle,
    pub local_group_root: Option<Handle>,
    pub ref_count: u32,
    pub st_dev: u64,
    pub st_ino: u64,
    pub file_offset: i64,
}

impl SoInfo {
    // === Flag helpers ===

    pub fn is_linked(&self) -> bool {
        self.flags & FLAG_LINKED != 0
    }

    pub fn set_linked(&mut self) {
        self.flags |= FLAG_LINKED;
    }

    pub fn is_image_linked(&self) -> bool {
        self.flags & FLAG_IMAGE_LINKED != 0
    }

    pub fn set_image_linked(&mut self) {
        self.flags |= FLAG_IMAGE_LINKED;
    }

    pub fn is_gnu_hash(&self) -> bool {
        self.flags & FLAG_GNU_HASH != 0
    }

    pub fn is_main_executable(&self) -> bool {
        self.flags & FLAG_EXE != 0
    }

    pub fn is_linker(&self) -> bool {
        self.flags & FLAG_LINKER != 0
    }

    pub fn set_main_executable(&mut self) {
        self.flags |= FLAG_EXE;
    }

    pub fn set_linker_flag(&mut self) {
        self.flags |= FLAG_LINKER;
    }

    pub fn is_mapped_by_caller(&self) -> bool {
        self.flags & FLAG_MAPPED_BY_CALLER != 0
    }

    pub fn set_mapped_by_caller(&mut self, mapped: bool) {
        if mapped {
            self.flags |= FLAG_MAPPED_BY_CALLER;
        } else {
            self.flags &= !FLAG_MAPPED_BY_CALLER;
        }
    }

    pub fn set_gnu_hash_flag(&mut self) {
        self.flags |= FLAG_GNU_HASH;
    }

    // === Linked state ===

    pub fn can_unload(&self) -> bool {
        !self.is_linked()
            || (self.rtld_flags & (RTLD_NODELETE | RTLD_GLOBAL)) == 0
    }

    // === DT flags ===

    pub fn set_dt_flags_1(&mut self, dt_flags_1: u32) {
        if dt_flags_1 & DF_1_GLOBAL != 0 {
            self.rtld_flags |= RTLD_GLOBAL;
        }
        if dt_flags_1 & DF_1_NODELETE != 0 {
            self.rtld_flags |= RTLD_NODELETE;
        }
        self.dt_flags_1 = dt_flags_1 as u64;
    }

    pub fn set_nodelete(&mut self) {
        self.rtld_flags |= RTLD_NODELETE;
    }

    // === Ref counting ===

    pub fn get_ref_count(&self) -> u32 {
        self.ref_count
    }

    pub fn increment_ref_count(&mut self) -> u32 {
        self.ref_count += 1;
        self.ref_count
    }

    pub fn decrement_ref_count(&mut self) -> u32 {
        self.ref_count = self.ref_count.saturating_sub(1);
        self.ref_count
    }

    // === Children / parents ===

    pub fn add_child(&mut self, child: Handle) {
        self.children.push(child);
    }

    pub fn remove_all_links(&mut self) {
        self.children.clear();
        self.parents.clear();
        self.secondary_namespaces.clear();
        self.primary_namespace = None;
    }

    // === Constructors / destructors ===

    pub unsafe fn call_pre_init_constructors(&self) {
        if let Some((addr, count)) = self.preinit_array {
            if count == 0 {
                return;
            }
            let n = count / size_of::<usize>();
            let arr = unsafe { std::slice::from_raw_parts(addr as *const unsafe extern "C" fn(), n) };
            for &f in arr {
                if f as usize != 0 && f as usize != usize::MAX {
                    f();
                }
            }
        }
    }

    /// Call init functions (DT_INIT before DT_INIT_ARRAY).
    pub unsafe fn call_init_functions(&self) {
        if let Some(addr) = self.init {
            if addr != 0 && addr != usize::MAX {
                let f: unsafe extern "C" fn(i32, *mut *mut i8, *mut *mut i8) =
                    unsafe { std::mem::transmute(addr) };
                unsafe {
                    f(0, std::ptr::null_mut(), std::ptr::null_mut());
                }
            }
        }
        if let Some((addr, count)) = self.init_array {
            if count == 0 {
                return;
            }
            let n = count / size_of::<usize>();
            if n == 0 {
                return;
            }
            let arr = unsafe { std::slice::from_raw_parts(addr as *const unsafe extern "C" fn(), n) };
            for &f in arr {
                if f as usize != 0 && f as usize != usize::MAX {
                    f();
                }
            }
        }
    }

    /// Call fini functions (DT_FINI_ARRAY in reverse, then DT_FINI).
    pub unsafe fn call_fini_functions(&self) {
        if let Some((addr, count)) = self.fini_array {
            if count == 0 {
                return;
            }
            let n = count / size_of::<usize>();
            if n == 0 {
                return;
            }
            let arr = unsafe { std::slice::from_raw_parts(addr as *const unsafe extern "C" fn(), n) };
            for i in (0..n).rev() {
                let f = arr[i];
                if f as usize != 0 && f as usize != usize::MAX {
                    f();
                }
            }
        }
        if let Some(addr) = self.fini {
            if addr != 0 && addr != usize::MAX {
                let f: unsafe extern "C" fn() = unsafe { std::mem::transmute(addr) };
                unsafe {
                    f();
                }
            }
        }
    }

    // === Handle ===

    pub fn get_handle(&self) -> Handle {
        self.handle
    }

    // === Realpath / soname (with old_name compat) ===

    pub fn get_realpath(&self) -> &str {
        &self.name
    }

    pub fn set_realpath(&mut self, path: &str) {
        self.name = path.to_string();
    }

    pub fn get_soname(&self) -> &str {
        &self.soname
    }

    pub fn set_soname(&mut self, soname: &str) {
        self.soname = soname.to_string();
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum RelocType {
    Rel,
    Rela,
}

impl Default for RelocType {
    fn default() -> Self {
        RelocType::Rela
    }
}

#[derive(Clone, Default, Debug)]
pub struct TlsSegment {
    pub size: usize,
    pub alignment: usize,
    pub init_ptr: usize,
    pub init_size: usize,
}

// Bionic constants used by soinfo methods
const RTLD_GLOBAL: i32 = 0x00100;
const RTLD_NODELETE: i32 = 0x01000;
const DF_1_GLOBAL: u32 = 0x0000_0002;
const DF_1_NODELETE: u32 = 0x0000_0008;

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_lock<F: FnOnce()>(f: F) {
        let _guard = TEST_MUTEX.lock().unwrap();
        f();
    }

    // --- flag helpers ---

    #[test]
    fn test_flags_default() {
        with_lock(|| {
            let si = SoInfo::default();
            assert!(!si.is_linked());
            assert!(!si.is_image_linked());
            assert!(!si.is_gnu_hash());
            assert!(!si.is_main_executable());
            assert!(!si.is_linker());
            assert!(!si.is_mapped_by_caller());
        });
    }

    #[test]
    fn test_set_linked() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_linked();
            assert!(si.is_linked());
            assert!(si.flags & FLAG_LINKED != 0);
        });
    }

    #[test]
    fn test_set_image_linked() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_image_linked();
            assert!(si.is_image_linked());
        });
    }

    #[test]
    fn test_set_main_executable() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_main_executable();
            assert!(si.is_main_executable());
            assert!(!si.is_linker());
        });
    }

    #[test]
    fn test_set_linker_flag() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_linker_flag();
            assert!(si.is_linker());
        });
    }

    #[test]
    fn test_gnu_hash_flag() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_gnu_hash_flag();
            assert!(si.is_gnu_hash());
        });
    }

    #[test]
    fn test_mapped_by_caller() {
        with_lock(|| {
            let mut si = SoInfo::default();
            assert!(!si.is_mapped_by_caller());
            si.set_mapped_by_caller(true);
            assert!(si.is_mapped_by_caller());
            si.set_mapped_by_caller(false);
            assert!(!si.is_mapped_by_caller());
        });
    }

    // --- can_unload ---

    #[test]
    fn test_can_unload_unlinked() {
        with_lock(|| {
            let si = SoInfo::default();
            assert!(si.can_unload());
        });
    }

    #[test]
    fn test_can_unload_linked_without_restrictions() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_linked();
            assert!(si.can_unload());
        });
    }

    #[test]
    fn test_can_unload_linked_nodelete() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_linked();
            si.set_nodelete();
            assert!(!si.can_unload());
        });
    }

    #[test]
    fn test_can_unload_linked_global() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_linked();
            si.rtld_flags = RTLD_GLOBAL;
            assert!(!si.can_unload());
        });
    }

    // --- set_dt_flags_1 ---

    #[test]
    fn test_set_dt_flags_1_global() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_dt_flags_1(DF_1_GLOBAL);
            assert!(si.rtld_flags & RTLD_GLOBAL != 0);
            assert_eq!(si.dt_flags_1, DF_1_GLOBAL as u64);
        });
    }

    #[test]
    fn test_set_dt_flags_1_nodelete() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_dt_flags_1(DF_1_NODELETE);
            assert!(si.rtld_flags & RTLD_NODELETE != 0);
        });
    }

    #[test]
    fn test_set_dt_flags_1_both() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_dt_flags_1(DF_1_GLOBAL | DF_1_NODELETE);
            assert!(si.rtld_flags & (RTLD_GLOBAL | RTLD_NODELETE) != 0);
        });
    }

    #[test]
    fn test_set_nodelete() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.set_nodelete();
            assert!(si.rtld_flags & RTLD_NODELETE != 0);
        });
    }

    // --- ref counting ---

    #[test]
    fn test_ref_count_default() {
        with_lock(|| {
            let si = SoInfo::default();
            assert_eq!(si.get_ref_count(), 0);
        });
    }

    #[test]
    fn test_increment_ref_count() {
        with_lock(|| {
            let mut si = SoInfo::default();
            assert_eq!(si.increment_ref_count(), 1);
            assert_eq!(si.get_ref_count(), 1);
            si.increment_ref_count();
            assert_eq!(si.get_ref_count(), 2);
        });
    }

    #[test]
    fn test_decrement_ref_count() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.ref_count = 3;
            assert_eq!(si.decrement_ref_count(), 2);
            assert_eq!(si.get_ref_count(), 2);
        });
    }

    #[test]
    fn test_decrement_ref_count_never_underflows() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.decrement_ref_count();
            assert_eq!(si.get_ref_count(), 0);
            si.decrement_ref_count();
            assert_eq!(si.get_ref_count(), 0);
        });
    }

    // --- children / parents ---

    #[test]
    fn test_add_child() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.add_child(42);
            assert_eq!(si.children, vec![42]);
            si.add_child(99);
            assert_eq!(si.children, vec![42, 99]);
        });
    }

    #[test]
    fn test_remove_all_links() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.children = vec![1, 2, 3];
            si.parents = vec![4, 5];
            si.secondary_namespaces = vec!["ns1".into()];
            si.primary_namespace = Some("default".into());
            si.remove_all_links();
            assert!(si.children.is_empty());
            assert!(si.parents.is_empty());
            assert!(si.secondary_namespaces.is_empty());
            assert!(si.primary_namespace.is_none());
        });
    }

    // --- realpath / soname ---

    #[test]
    fn test_realpath_get_set() {
        with_lock(|| {
            let mut si = SoInfo::default();
            assert_eq!(si.get_realpath(), "");
            si.set_realpath("/tmp/libfoo.so");
            assert_eq!(si.get_realpath(), "/tmp/libfoo.so");
            assert_eq!(si.name, "/tmp/libfoo.so");
        });
    }

    #[test]
    fn test_soname_get_set() {
        with_lock(|| {
            let mut si = SoInfo::default();
            assert_eq!(si.get_soname(), "");
            si.set_soname("libfoo.so");
            assert_eq!(si.get_soname(), "libfoo.so");
            assert_eq!(si.soname, "libfoo.so");
        });
    }

    // --- get_handle ---

    #[test]
    fn test_get_handle() {
        with_lock(|| {
            let mut si = SoInfo::default();
            si.handle = 42;
            assert_eq!(si.get_handle(), 42);
        });
    }
}

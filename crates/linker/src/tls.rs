use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Mutex;

use crate::soinfo::{SoInfo, TlsSegment};

pub const TLS_GENERATION_NONE: usize = 0;
pub const TLS_GENERATION_FIRST: usize = 1;
pub const TLS_UNINITIALIZED_MODULE_ID: usize = 0;

fn tls_module_id_to_idx(id: usize) -> usize {
    id - 1
}

fn tls_module_idx_to_id(idx: usize) -> usize {
    idx + 1
}

#[derive(Clone, Debug)]
pub struct TlsModule {
    pub segment: TlsSegment,
    pub static_offset: usize,
    pub first_generation: usize,
    pub soinfo_base: Option<usize>,
}

impl Default for TlsModule {
    fn default() -> Self {
        TlsModule {
            segment: TlsSegment::default(),
            static_offset: usize::MAX,
            first_generation: TLS_GENERATION_NONE,
            soinfo_base: None,
        }
    }
}

static G_STATIC_TLS_FINISHED: AtomicBool = AtomicBool::new(false);
static NEXT_GENERATION: AtomicUsize = AtomicUsize::new(TLS_GENERATION_FIRST);
static G_TLS_MODULES: Mutex<Vec<TlsModule>> = Mutex::new(Vec::new());

fn register_tls_module(soinfo_base: usize, segment: &TlsSegment) -> usize {
    let new_generation = NEXT_GENERATION.fetch_add(1, Ordering::Release) + 1;
    let mut modules = G_TLS_MODULES.lock().unwrap();
    let idx = {
        let mut found = modules.len();
        for (i, m) in modules.iter().enumerate() {
            if m.soinfo_base.is_none() {
                found = i;
                break;
            }
        }
        if found == modules.len() {
            modules.push(TlsModule::default());
        }
        found
    };
    modules[idx] = TlsModule {
        segment: segment.clone(),
        static_offset: usize::MAX,
        first_generation: new_generation,
        soinfo_base: Some(soinfo_base),
    };
    tls_module_idx_to_id(idx)
}

fn unregister_tls_module(soinfo_base: usize) {
    let mut modules = G_TLS_MODULES.lock().unwrap();
    for m in modules.iter_mut() {
        if m.soinfo_base == Some(soinfo_base) {
            *m = TlsModule::default();
            return;
        }
    }
}

pub fn get_tls_module(module_id: usize) -> Option<TlsModule> {
    if module_id == TLS_UNINITIALIZED_MODULE_ID {
        return None;
    }
    let module_idx = tls_module_id_to_idx(module_id);
    let modules = G_TLS_MODULES.lock().unwrap();
    if module_idx < modules.len() {
        let m = &modules[module_idx];
        if m.soinfo_base.is_some() {
            return Some(m.clone());
        }
    }
    None
}

pub fn register_soinfo_tls(si: &mut SoInfo) {
    let segment = match &si.tls_segment {
        Some(s) => s.clone(),
        None => return,
    };
    if si.tls_module_id != TLS_UNINITIALIZED_MODULE_ID {
        return;
    }
    let soinfo_base = si.base;
    si.tls_module_id = register_tls_module(soinfo_base, &segment);
}

pub fn unregister_soinfo_tls(si: &mut SoInfo) {
    if si.tls_segment.is_none() || si.tls_module_id == TLS_UNINITIALIZED_MODULE_ID {
        return;
    }
    unregister_tls_module(si.base);
    si.tls_module_id = TLS_UNINITIALIZED_MODULE_ID;
}

pub struct TlsModuleInfo {
    pub module_id: usize,
    pub segment: TlsSegment,
    pub static_offset: usize,
    pub first_generation: usize,
    pub soinfo_base: Option<usize>,
}

pub fn get_all_tls_modules() -> Vec<TlsModuleInfo> {
    let modules = G_TLS_MODULES.lock().unwrap();
    modules
        .iter()
        .enumerate()
        .filter(|(_, m)| m.soinfo_base.is_some())
        .map(|(idx, m)| TlsModuleInfo {
            module_id: tls_module_idx_to_id(idx),
            segment: m.segment.clone(),
            static_offset: m.static_offset,
            first_generation: m.first_generation,
            soinfo_base: m.soinfo_base,
        })
        .collect()
}

pub fn linker_setup_exe_static_tls(_progname: &str) {
    // On Linux, static TLS for the main executable is handled by libc.
    // This is a no-op outside Bionic.
}

pub fn linker_finalize_static_tls() {
    G_STATIC_TLS_FINISHED.store(true, Ordering::Release);
}

pub fn static_tls_finished() -> bool {
    G_STATIC_TLS_FINISHED.load(Ordering::Acquire)
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_lock<F: FnOnce()>(f: F) {
        let _guard = TEST_MUTEX.lock().unwrap();
        f();
    }

    fn make_test_soinfo(base: usize, tls_size: usize) -> SoInfo {
        let mut si = SoInfo {
            name: format!("lib{:x}.so", base),
            base,
            ..Default::default()
        };
        if tls_size > 0 {
            si.tls_segment = Some(TlsSegment {
                size: tls_size,
                alignment: 64,
                init_ptr: 0x1234,
                init_size: if tls_size > 16 { 16 } else { tls_size },
            });
        }
        si
    }

    #[test]
    fn test_register_tls_module() {
        with_lock(|| {
            let mut si = make_test_soinfo(0x1000, 256);
            assert_eq!(si.tls_module_id, TLS_UNINITIALIZED_MODULE_ID);
            register_soinfo_tls(&mut si);
            assert!(si.tls_module_id != TLS_UNINITIALIZED_MODULE_ID);
            let module = get_tls_module(si.tls_module_id);
            assert!(module.is_some());
            let module = module.unwrap();
            assert_eq!(module.segment.size, 256);
            assert_eq!(module.segment.alignment, 64);
            assert_eq!(module.segment.init_size, 16);
            assert_eq!(module.soinfo_base, Some(0x1000));
        });
    }

    #[test]
    fn test_unregister_tls_module() {
        with_lock(|| {
            let mut si = make_test_soinfo(0x2000, 128);
            register_soinfo_tls(&mut si);
            let module_id = si.tls_module_id;
            assert!(get_tls_module(module_id).is_some());
            unregister_soinfo_tls(&mut si);
            assert_eq!(si.tls_module_id, TLS_UNINITIALIZED_MODULE_ID);
            assert!(get_tls_module(module_id).is_none());
        });
    }

    #[test]
    fn test_no_tls_segment() {
        with_lock(|| {
            let mut si = make_test_soinfo(0x3000, 0);
            si.tls_segment = None;
            register_soinfo_tls(&mut si);
            assert_eq!(si.tls_module_id, TLS_UNINITIALIZED_MODULE_ID);
        });
    }

    #[test]
    fn test_double_register_is_noop() {
        with_lock(|| {
            let mut si = make_test_soinfo(0x4000, 64);
            register_soinfo_tls(&mut si);
            let first_id = si.tls_module_id;
            register_soinfo_tls(&mut si);
            assert_eq!(si.tls_module_id, first_id);
        });
    }

    #[test]
    fn test_double_unregister_is_noop() {
        with_lock(|| {
            let mut si = make_test_soinfo(0x5000, 32);
            register_soinfo_tls(&mut si);
            unregister_soinfo_tls(&mut si);
            unregister_soinfo_tls(&mut si);
            assert_eq!(si.tls_module_id, TLS_UNINITIALIZED_MODULE_ID);
        });
    }

    #[test]
    fn test_multiple_modules() {
        with_lock(|| {
            let mut si1 = make_test_soinfo(0x6000, 64);
            let mut si2 = make_test_soinfo(0x7000, 128);
            let mut si3 = make_test_soinfo(0x8000, 256);
            register_soinfo_tls(&mut si1);
            register_soinfo_tls(&mut si2);
            register_soinfo_tls(&mut si3);
            assert!(si1.tls_module_id != TLS_UNINITIALIZED_MODULE_ID);
            assert!(si2.tls_module_id != TLS_UNINITIALIZED_MODULE_ID);
            assert!(si3.tls_module_id != TLS_UNINITIALIZED_MODULE_ID);
            assert_ne!(si1.tls_module_id, si2.tls_module_id);
            assert_ne!(si2.tls_module_id, si3.tls_module_id);
            assert_ne!(si1.tls_module_id, si3.tls_module_id);
            assert!(get_tls_module(si1.tls_module_id).is_some());
            assert!(get_tls_module(si2.tls_module_id).is_some());
            assert!(get_tls_module(si3.tls_module_id).is_some());

            unregister_soinfo_tls(&mut si2);
            assert!(get_tls_module(si2.tls_module_id).is_none());
            assert!(get_tls_module(si1.tls_module_id).is_some());
            assert!(get_tls_module(si3.tls_module_id).is_some());
        });
    }

    #[test]
    fn test_tls_constants() {
        with_lock(|| {
            assert_eq!(tls_module_id_to_idx(1), 0);
            assert_eq!(tls_module_id_to_idx(2), 1);
            assert_eq!(tls_module_idx_to_id(0), 1);
            assert_eq!(tls_module_idx_to_id(1), 2);
        });
    }

    #[test]
    fn test_static_tls_finished() {
        with_lock(|| {
            assert!(!static_tls_finished());
            linker_finalize_static_tls();
            assert!(static_tls_finished());
            G_STATIC_TLS_FINISHED.store(false, Ordering::Release);
            assert!(!static_tls_finished());
        });
    }

    #[test]
    fn test_get_nonexistent_module() {
        with_lock(|| {
            assert!(get_tls_module(999).is_none());
        });
    }

    #[test]
    fn test_module_reuse_after_unregister() {
        with_lock(|| {
            let mut si1 = make_test_soinfo(0x9000, 64);
            let mut si2 = make_test_soinfo(0xA000, 64);
            register_soinfo_tls(&mut si1);
            let id1 = si1.tls_module_id;
            unregister_soinfo_tls(&mut si1);
            register_soinfo_tls(&mut si2);
            assert_eq!(si2.tls_module_id, id1);
        });
    }
}

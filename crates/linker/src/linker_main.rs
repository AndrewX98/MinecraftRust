use std::path::Path;
use std::sync::Mutex;

use crate::Handle;

// ============================================================================
// Solist — ordered list of loaded library handles
// Mirrors the C++ `solist` linked list in linker_main.cpp.
// ============================================================================

static SOLIST: Mutex<Vec<Handle>> = Mutex::new(Vec::new());
static SOMAIN: Mutex<Option<Handle>> = Mutex::new(None);
static SOLINKER: Mutex<Option<Handle>> = Mutex::new(None);
static VDSO: Mutex<Option<Handle>> = Mutex::new(None);

pub fn solist_init() {
    let mut list = SOLIST.lock().unwrap();
    list.clear();
    // solist_init creates two entries: "linker.so" and "linker_main"
    list.push(1);
    list.push(2);
    let mut solinker = SOLINKER.lock().unwrap();
    *solinker = Some(1);
}

pub fn solist_add_soinfo(handle: Handle) {
    let mut list = SOLIST.lock().unwrap();
    if !list.contains(&handle) {
        list.push(handle);
    }
}

pub fn solist_remove_soinfo(handle: Handle) -> bool {
    let mut list = SOLIST.lock().unwrap();
    let pos = match list.iter().position(|&h| h == handle) {
        Some(p) => p,
        None => return false,
    };
    list.remove(pos);
    true
}

pub fn solist_get_head() -> Option<Handle> {
    SOLIST.lock().unwrap().first().copied()
}

pub fn solist_get_somain() -> Option<Handle> {
    *SOMAIN.lock().unwrap()
}

pub fn solist_get_vdso() -> Option<Handle> {
    *VDSO.lock().unwrap()
}

pub fn solist_set_somain(handle: Handle) {
    *SOMAIN.lock().unwrap() = Some(handle);
}

pub fn solist_set_vdso(handle: Handle) {
    *VDSO.lock().unwrap() = Some(handle);
}

/// Return a snapshot of the current solist contents (for testing/inspection).
pub fn solist_snapshot() -> Vec<Handle> {
    SOLIST.lock().unwrap().clone()
}

// ============================================================================
// Global flags (g_is_ldd, g_ld_debug_verbosity)
// ============================================================================

static G_IS_LDD: Mutex<bool> = Mutex::new(false);
static G_LD_DEBUG_VERBOSITY: Mutex<i32> = Mutex::new(-1);

pub fn set_is_ldd(val: bool) {
    *G_IS_LDD.lock().unwrap() = val;
}

pub fn is_ldd() -> bool {
    *G_IS_LDD.lock().unwrap()
}

pub fn set_ld_debug_verbosity(val: i32) {
    *G_LD_DEBUG_VERBOSITY.lock().unwrap() = val;
}

pub fn ld_debug_verbosity() -> i32 {
    *G_LD_DEBUG_VERBOSITY.lock().unwrap()
}

// ============================================================================
// LD_PRELOAD names (g_ld_preload_names, g_ld_preloads)
// ============================================================================

static G_LD_PRELOAD_NAMES: Mutex<Vec<String>> = Mutex::new(Vec::new());
static G_LD_PRELOADS: Mutex<Vec<Handle>> = Mutex::new(Vec::new());

pub fn ld_preload_names() -> Vec<String> {
    G_LD_PRELOAD_NAMES.lock().unwrap().clone()
}

pub fn ld_preloads() -> Vec<Handle> {
    G_LD_PRELOADS.lock().unwrap().clone()
}

pub fn set_ld_preloads(preloads: Vec<Handle>) {
    *G_LD_PRELOADS.lock().unwrap() = preloads;
}

// ============================================================================
// LD_LIBRARY_PATH (stored until namespace is available)
// ============================================================================

static G_LD_LIBRARY_PATH: Mutex<Vec<String>> = Mutex::new(Vec::new());

pub fn ld_library_path() -> Vec<String> {
    G_LD_LIBRARY_PATH.lock().unwrap().clone()
}

// ============================================================================
// Path parsing helpers (replaces linker_utils.h split_path / resolve_paths)
// ============================================================================

/// Split a path string by characters in delimiters, ignoring empty segments.
fn split_path<'a>(path: &'a str, delimiters: &str) -> Vec<String> {
    if path.is_empty() {
        return Vec::new();
    }
    path.split(|c| delimiters.contains(c))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

/// Resolve each path to an absolute canonical path using the filesystem.
/// Non-existent paths are kept as-is (not removed).
fn resolve_paths(paths: &[String]) -> Vec<String> {
    paths
        .iter()
        .map(|p| {
            Path::new(p)
                .canonicalize()
                .map(|pb| pb.to_string_lossy().into_owned())
                .unwrap_or_else(|_| p.clone())
        })
        .collect()
}

// ============================================================================
// Exported parsing functions (matching linker_main.cpp)
// ============================================================================

pub fn parse_path(path: &str, delimiters: &str) -> Vec<String> {
    let paths = split_path(path, delimiters);
    resolve_paths(&paths)
}

pub fn parse_ld_library_path(path: &str) {
    let resolved = parse_path(path, ":");
    let mut store = G_LD_LIBRARY_PATH.lock().unwrap();
    *store = resolved;
}

pub fn parse_ld_preload(path: Option<&str>) {
    let mut names = G_LD_PRELOAD_NAMES.lock().unwrap();
    names.clear();
    if let Some(p) = path {
        *names = p
            .split(|c: char| c == ' ' || c == ':')
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect();
    }
}

// ============================================================================
// call_ifunc_resolvers — no-op on Linux (no __rela_iplt symbols in Rust linker)
// ============================================================================

pub fn call_ifunc_resolvers(_load_bias: usize) {
    // On Linux without Android's linker-defined __rela_iplt_start/__rela_iplt_end
    // symbols, IFUNC resolvers are handled by glibc's dynamic linker, not ours.
    // This is a no-op stub.
}

// ============================================================================
// No-op stubs for #if 0'd functions (declared in linker_main.h but disabled)
// ============================================================================

/// Stub for the main linker init function (disabled via #if 0 in C++).
pub fn linker_main_stub() {
    log::warn!("linker_main_stub: linker_main() is not implemented (disabled via #if 0)");
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Serializes tests that modify global state, preventing races
    /// between parallel test threads in the same process.
    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_lock<F: FnOnce()>(f: F) {
        let _guard = TEST_MUTEX.lock().unwrap();
        f();
    }

    // --- solist tests ---

    #[test]
    fn test_solist_init_creates_two_entries() {
        with_lock(|| {
            solist_init();
            let snap = solist_snapshot();
            assert_eq!(snap.len(), 2);
            assert_eq!(snap[0], 1);
            assert_eq!(snap[1], 2);
            assert_eq!(solist_get_head(), Some(1));
        });
    }

    #[test]
    fn test_solist_add_and_remove() {
        with_lock(|| {
            solist_init();
            solist_add_soinfo(10);
            let snap = solist_snapshot();
            assert!(snap.contains(&10));
            assert!(solist_remove_soinfo(10));
            let snap = solist_snapshot();
            assert!(!snap.contains(&10));
        });
    }

    #[test]
    fn test_solist_remove_nonexistent() {
        with_lock(|| {
            solist_init();
            assert!(!solist_remove_soinfo(999));
        });
    }

    #[test]
    fn test_solist_add_duplicate() {
        with_lock(|| {
            solist_init();
            solist_add_soinfo(5);
            solist_add_soinfo(5);
            let snap = solist_snapshot();
            assert_eq!(snap.iter().filter(|&&h| h == 5).count(), 1);
        });
    }

    #[test]
    fn test_solist_somain_vdso() {
        with_lock(|| {
            solist_init();
            assert_eq!(solist_get_somain(), None);
            assert_eq!(solist_get_vdso(), None);
            solist_set_somain(100);
            solist_set_vdso(200);
            assert_eq!(solist_get_somain(), Some(100));
            assert_eq!(solist_get_vdso(), Some(200));
        });
    }

    #[test]
    fn test_solist_get_head_on_empty() {
        with_lock(|| {
            let mut list = SOLIST.lock().unwrap();
            list.clear();
            drop(list);
            assert_eq!(solist_get_head(), None);
            solist_init();
        });
    }

    // --- globals tests ---

    #[test]
    fn test_is_ldd_default_false() {
        with_lock(|| {
            assert!(!is_ldd());
        });
    }

    #[test]
    fn test_is_ldd_set() {
        with_lock(|| {
            set_is_ldd(true);
            assert!(is_ldd());
            set_is_ldd(false);
            assert!(!is_ldd());
        });
    }

    #[test]
    fn test_ld_debug_verbosity_default_neg1() {
        with_lock(|| {
            assert_eq!(ld_debug_verbosity(), -1);
        });
    }

    #[test]
    fn test_ld_debug_verbosity_set() {
        with_lock(|| {
            set_ld_debug_verbosity(2);
            assert_eq!(ld_debug_verbosity(), 2);
            set_ld_debug_verbosity(-1);
        });
    }

    #[test]
    fn test_ld_preload_names_empty_by_default() {
        with_lock(|| {
            assert!(ld_preload_names().is_empty());
        });
    }

    // --- path parsing tests ---

    #[test]
    fn test_split_path_empty() {
        assert!(split_path("", ":").is_empty());
    }

    #[test]
    fn test_split_path_single() {
        let parts = split_path("/usr/lib", ":");
        assert_eq!(parts, vec!["/usr/lib"]);
    }

    #[test]
    fn test_split_path_multiple() {
        let parts = split_path("/usr/lib:/lib:/usr/local/lib", ":");
        assert_eq!(parts, vec!["/usr/lib", "/lib", "/usr/local/lib"]);
    }

    #[test]
    fn test_split_path_ignores_empty_segments() {
        let parts = split_path("::/usr/lib::/lib:", ":");
        assert_eq!(parts, vec!["/usr/lib", "/lib"]);
    }

    #[test]
    fn test_parse_path_simple() {
        let resolved = parse_path("/usr/lib:/tmp/nonexistent_path_xyz", ":");
        let usr_lib = std::fs::canonicalize("/usr/lib")
            .unwrap()
            .to_string_lossy()
            .into_owned();
        assert!(resolved.contains(&usr_lib));
        assert!(resolved.contains(&"/tmp/nonexistent_path_xyz".to_string()));
    }

    #[test]
    fn test_parse_ld_library_path() {
        with_lock(|| {
            parse_ld_library_path("/usr/lib:/usr/local/lib");
            assert!(ld_library_path().len() >= 2);
        });
    }

    #[test]
    fn test_parse_ld_preload_empty() {
        with_lock(|| {
            parse_ld_preload(None);
            assert!(ld_preload_names().is_empty());
        });
    }

    #[test]
    fn test_parse_ld_preload_space_delimited() {
        with_lock(|| {
            parse_ld_preload(Some("libfoo.so libbar.so"));
            assert_eq!(ld_preload_names(), vec!["libfoo.so", "libbar.so"]);
        });
    }

    #[test]
    fn test_parse_ld_preload_colon_delimited() {
        with_lock(|| {
            parse_ld_preload(Some("libfoo.so:libbar.so"));
            assert_eq!(ld_preload_names(), vec!["libfoo.so", "libbar.so"]);
        });
    }

    #[test]
    fn test_parse_ld_preload_mixed() {
        with_lock(|| {
            parse_ld_preload(Some("libfoo.so:libbar.so libbaz.so"));
            assert_eq!(ld_preload_names(), vec!["libfoo.so", "libbar.so", "libbaz.so"]);
        });
    }

    #[test]
    fn test_parse_ld_preload_ignores_empty() {
        with_lock(|| {
            parse_ld_preload(Some("libfoo.so::: libbar.so"));
            assert_eq!(ld_preload_names(), vec!["libfoo.so", "libbar.so"]);
        });
    }

    // --- call_ifunc_resolvers ---

    #[test]
    fn test_call_ifunc_resolvers_no_panic() {
        call_ifunc_resolvers(0);
        call_ifunc_resolvers(0x7f000000);
    }

    // --- preloads list ---

    #[test]
    fn test_ld_preloads_default_empty() {
        with_lock(|| {
            assert!(ld_preloads().is_empty());
        });
    }

    #[test]
    fn test_ld_preloads_set_and_get() {
        with_lock(|| {
            set_ld_preloads(vec![10, 20, 30]);
            assert_eq!(ld_preloads(), vec![10, 20, 30]);
            set_ld_preloads(vec![]);
        });
    }

    // --- linker_main_stub ---

    #[test]
    fn test_linker_main_stub_no_panic() {
        linker_main_stub();
    }
}

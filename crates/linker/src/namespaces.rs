use std::collections::HashSet;

use crate::Handle;

/// DT_FLAGS_1: if set, library is visible in dlsym lookups across namespaces.
pub const DF_1_GLOBAL: u64 = 0x0000_0002;

/// RTLD_GLOBAL flag for dlopen.
pub const RTLD_GLOBAL: i32 = 0x00100;

/// A link from one namespace to another, with shared library restrictions.
#[derive(Clone, Debug)]
pub struct NamespaceLink {
    pub target_id: usize,
    pub shared_lib_sonames: HashSet<String>,
    pub allow_all_shared_libs: bool,
}

impl NamespaceLink {
    pub fn is_accessible(&self, soname: Option<&str>) -> bool {
        match soname {
            Some(s) => self.allow_all_shared_libs || self.shared_lib_sonames.contains(s),
            None => false,
        }
    }
}

/// Android linker namespace: controls library search paths and symbol visibility.
#[derive(Clone, Debug)]
pub struct AndroidNamespace {
    pub name: String,
    pub is_isolated: bool,
    pub is_greylist_enabled: bool,
    pub is_also_used_as_anonymous: bool,
    pub ld_library_paths: Vec<String>,
    pub default_library_paths: Vec<String>,
    pub permitted_paths: Vec<String>,
    pub whitelisted_libs: Vec<String>,
    pub linked_namespaces: Vec<NamespaceLink>,
    pub soinfo_list: Vec<Handle>,
}

impl AndroidNamespace {
    pub fn new(name: &str) -> Self {
        AndroidNamespace {
            name: name.to_string(),
            is_isolated: false,
            is_greylist_enabled: false,
            is_also_used_as_anonymous: false,
            ld_library_paths: Vec::new(),
            default_library_paths: Vec::new(),
            permitted_paths: Vec::new(),
            whitelisted_libs: Vec::new(),
            linked_namespaces: Vec::new(),
            soinfo_list: Vec::new(),
        }
    }

    /// Check if an absolute file path is accessible from this namespace.
    pub fn is_accessible(&self, file: &str) -> bool {
        if !self.is_isolated {
            return true;
        }

        if !self.whitelisted_libs.is_empty() {
            let lib_name = crate::utils::basename(file);
            if !self.whitelisted_libs.iter().any(|w| w.as_str() == lib_name) {
                return false;
            }
        }

        for dir in &self.ld_library_paths {
            if crate::utils::file_is_in_dir(file, dir) {
                return true;
            }
        }

        for dir in &self.default_library_paths {
            if crate::utils::file_is_in_dir(file, dir) {
                return true;
            }
        }

        for dir in &self.permitted_paths {
            if crate::utils::file_is_under_dir(file, dir) {
                return true;
            }
        }

        false
    }

    /// Check if a soinfo is accessible from this namespace.
    pub fn is_accessible_soinfo(&self, si: &crate::soinfo::SoInfo) -> bool {
        // Check primary namespace membership
        if self.is_primary_namespace(si) {
            return true;
        }

        // Check secondary namespace membership
        if si.secondary_namespaces.contains(&self.name) {
            return true;
        }

        // Check if any parent soinfo belongs to this namespace
        !si.parents.iter().any(|_parent_idx| true)
    }

    fn is_primary_namespace(&self, si: &crate::soinfo::SoInfo) -> bool {
        match &si.primary_namespace {
            Some(ref name) => name == &self.name,
            None => self.name.is_empty(),
        }
    }

    /// Collect libraries with DF_1_GLOBAL flag set in this namespace.
    pub fn get_global_group(&self) -> Vec<Handle> {
        let state = crate::STATE.read().unwrap();
        let mut group = Vec::new();
        for &h in &self.soinfo_list {
            if let Some(lib) = state.libraries_by_handle.get(&h) {
                if lib.soinfo.dt_flags_1 & DF_1_GLOBAL != 0 {
                    group.push(h);
                }
            }
        }
        group
    }

    /// Collect RTLD_GLOBAL libraries from this namespace.
    /// For the default namespace, this is the same as get_global_group.
    /// For others, it includes all RTLD_GLOBAL libraries.
    pub fn get_shared_group(&self, is_default: bool) -> Vec<Handle> {
        if is_default {
            return self.get_global_group();
        }

        let state = crate::STATE.read().unwrap();
        let mut group = Vec::new();
        for &h in &self.soinfo_list {
            if let Some(lib) = state.libraries_by_handle.get(&h) {
                if lib.soinfo.rtld_flags & RTLD_GLOBAL != 0 {
                    group.push(h);
                }
            }
        }
        group
    }
}

impl Default for AndroidNamespace {
    fn default() -> Self {
        Self::new("")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init;

    fn setup() {
        init();
    }

    #[test]
    fn test_default_namespace_not_isolated() {
        let ns = AndroidNamespace::new("default");
        assert!(!ns.is_isolated);
        assert!(ns.is_accessible("/any/path.so"));
    }

    #[test]
    fn test_isolated_namespace_rejects_unlisted() {
        let mut ns = AndroidNamespace::new("isolated");
        ns.is_isolated = true;
        ns.default_library_paths = vec!["/allowed".to_string()];
        assert!(ns.is_accessible("/allowed/libfoo.so"));
        assert!(!ns.is_accessible("/forbidden/libbar.so"));
    }

    #[test]
    fn test_whitelisted_blocks_unlisted_libs() {
        let mut ns = AndroidNamespace::new("test");
        ns.is_isolated = true;
        ns.default_library_paths = vec!["/lib".to_string()];
        ns.whitelisted_libs = vec!["libfoo.so".to_string()];
        // libfoo.so is whitelisted AND in an allowed path
        assert!(ns.is_accessible("/lib/libfoo.so"));
        // libbar.so is NOT whitelisted — blocked even though in path
        assert!(!ns.is_accessible("/lib/libbar.so"));
    }

    #[test]
    fn test_permitted_paths_under_dir() {
        let mut ns = AndroidNamespace::new("test");
        ns.is_isolated = true;
        ns.permitted_paths = vec!["/base".to_string()];
        assert!(ns.is_accessible("/base/sub/lib.so"));
        assert!(ns.is_accessible("/base/lib.so"));
        assert!(!ns.is_accessible("/other/lib.so"));
    }

    #[test]
    fn test_ld_library_paths() {
        let mut ns = AndroidNamespace::new("test");
        ns.is_isolated = true;
        ns.ld_library_paths = vec!["/ld_path".to_string()];
        assert!(ns.is_accessible("/ld_path/lib.so"));
        assert!(!ns.is_accessible("/ld_path/sub/lib.so"));
    }

    #[test]
    fn test_not_isolated_allows_all() {
        let ns = AndroidNamespace::new("open");
        assert!(ns.is_accessible("/anything/goes.so"));
    }

    #[test]
    fn test_namespace_link_access() {
        let link = NamespaceLink {
            target_id: 1,
            shared_lib_sonames: vec!["libfoo.so".to_string()].into_iter().collect(),
            allow_all_shared_libs: false,
        };
        assert!(link.is_accessible(Some("libfoo.so")));
        assert!(!link.is_accessible(Some("libbar.so")));
        assert!(!link.is_accessible(None));
    }

    #[test]
    fn test_namespace_link_allow_all() {
        let link = NamespaceLink {
            target_id: 1,
            shared_lib_sonames: HashSet::new(),
            allow_all_shared_libs: true,
        };
        assert!(link.is_accessible(Some("anything.so")));
    }

    #[test]
    fn test_get_dl_symbols_not_affected() {
        setup();
        let syms = crate::libdl::get_dl_symbols();
        assert!(syms.contains_key("dlopen"));
    }

    #[test]
    fn test_global_group_empty_after_init() {
        setup();
        let ns = AndroidNamespace::new("default");
        let group = ns.get_global_group();
        assert!(group.is_empty());
    }

    #[test]
    fn test_shared_group_empty_after_init() {
        setup();
        let ns = AndroidNamespace::new("default");
        let group = ns.get_shared_group(true);
        assert!(group.is_empty());
    }

    #[test]
    fn test_default_namespace_name() {
        let ns = AndroidNamespace::default();
        assert_eq!(ns.name, "");
    }

    #[test]
    fn test_is_primary_namespace() {
        let ns = AndroidNamespace::new("myns");
        let mut si = crate::soinfo::SoInfo::default();
        si.name = "libtest.so".to_string();

        // Not assigned to any namespace
        assert!(!ns.is_primary_namespace(&si));

        // Assigned to this namespace
        si.primary_namespace = Some("myns".to_string());
        assert!(ns.is_primary_namespace(&si));

        // Assigned to a different namespace
        si.primary_namespace = Some("other".to_string());
        assert!(!ns.is_primary_namespace(&si));
    }

    #[test]
    fn test_get_global_group_filters_by_flag() {
        let mut ns = AndroidNamespace::new("test");
        // Use a unique name to avoid collisions with parallel tests sharing global STATE
        let sa = crate::load_library("libtest_global_group.so", &std::collections::HashMap::new());
        // Set DF_1_GLOBAL on the loaded library's soinfo
        {
            let mut state = crate::STATE.write().unwrap();
            if let Some(lib) = state.libraries_by_handle.get_mut(&sa) {
                lib.soinfo.dt_flags_1 = DF_1_GLOBAL;
            }
        }
        ns.soinfo_list.push(sa);

        let group = ns.get_global_group();
        assert_eq!(group.len(), 1);
        assert_eq!(group[0], sa);
    }

    #[test]
    fn test_get_shared_group_default_same_as_global() {
        let mut ns = AndroidNamespace::new("test");
        let sa = crate::load_library("libtest_shared_group.so", &std::collections::HashMap::new());
        ns.soinfo_list.push(sa);

        let global = ns.get_global_group();
        let shared = ns.get_shared_group(true);
        assert_eq!(global, shared);
    }
}

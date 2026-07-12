use std::sync::Mutex;

// === Error buffer (replaces linker_globals.cpp error buffer) ===

static LINKER_ERR_BUF: Mutex<String> = Mutex::new(String::new());

pub fn linker_get_error_buffer() -> String {
    LINKER_ERR_BUF.lock().unwrap().clone()
}

pub fn linker_set_error(msg: &str) {
    let mut buf = LINKER_ERR_BUF.lock().unwrap();
    buf.clear();
    buf.push_str(msg);
}

pub fn linker_get_error_buffer_size() -> usize {
    768
}

pub fn dl_err_internal(fmt: std::fmt::Arguments<'_>) {
    let s = format!("{}", fmt);
    let mut buf = LINKER_ERR_BUF.lock().unwrap();
    buf.clear();
    buf.push_str(&s);
}

#[macro_export]
macro_rules! dl_err {
    ($($arg:tt)*) => {
        $crate::linker_stubs::dl_err_internal(format_args!($($arg)*))
    };
}

pub fn dl_warn_internal(fmt: std::fmt::Arguments<'_>) {
    let s = format!("{}", fmt);
    log::warn!("linker: {}", s);
    eprintln!("WARNING: linker: {}", s);
}

#[macro_export]
macro_rules! dl_warn {
    ($($arg:tt)*) => {
        $crate::linker_stubs::dl_warn_internal(format_args!($($arg)*))
    };
}

/// RAII restorer that saves and restores the linker error buffer.
pub struct DlErrorRestorer {
    saved: String,
}

impl DlErrorRestorer {
    pub fn new() -> Self {
        DlErrorRestorer {
            saved: linker_get_error_buffer(),
        }
    }
}

impl Drop for DlErrorRestorer {
    fn drop(&mut self) {
        let mut buf = LINKER_ERR_BUF.lock().unwrap();
        buf.clear();
        buf.push_str(&self.saved);
    }
}

impl Default for DlErrorRestorer {
    fn default() -> Self {
        Self::new()
    }
}

// === argc/argv/envp (replaces linker_globals.cpp globals) ===

use std::sync::OnceLock;

static LINKER_ARGS: OnceLock<(usize, Vec<String>, Vec<String>)> = OnceLock::new();

pub fn linker_set_args(argc: usize, argv: Vec<String>, envp: Vec<String>) {
    LINKER_ARGS.set((argc, argv, envp)).ok();
}

pub fn linker_argc() -> usize {
    LINKER_ARGS.get().map(|a| a.0).unwrap_or(0)
}

pub fn linker_argv() -> &'static [String] {
    LINKER_ARGS.get().map(|a| a.1.as_slice()).unwrap_or(&[])
}

pub fn linker_envp() -> &'static [String] {
    LINKER_ARGS.get().map(|a| a.2.as_slice()).unwrap_or(&[])
}

// === Debuggerd stub (replaces linker_debuggerd_stub.cpp) ===

/// No-op debuggerd initialization for the linker.
pub fn linker_debuggerd_init() {
    // no-op on Linux
}

// === DL_WARN_documented_change (replaces linker_globals.cpp) ===

use crate::sdk_versions::get_application_target_sdk_version;

pub fn dl_warn_documented_change_internal(
    api_level: i32,
    doc_link: &str,
    fmt: std::fmt::Arguments<'_>,
) {
    let msg = format!("{}", fmt);
    let sdk_ver = get_application_target_sdk_version();
    let result = format!(
        "Warning: {} and will not work when the app moves to API level {} or later \
         (https://android.googlesource.com/platform/bionic/+/master/{}) \
         (allowing for now because this app's target API level is still {})",
        msg, api_level, doc_link, sdk_ver,
    );
    dl_warn_internal(format_args!("{}", result));
}

#[macro_export]
macro_rules! dl_warn_documented_change {
    ($api_level:expr, $doc_link:expr, $($arg:tt)*) => {
        $crate::linker_stubs::dl_warn_documented_change_internal(
            $api_level, $doc_link, format_args!($($arg)*)
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn with_lock<F: FnOnce()>(f: F) {
        let _guard = TEST_MUTEX.lock().unwrap();
        f();
    }

    #[test]
    fn test_linker_debuggerd_init_no_panic() {
        linker_debuggerd_init();
    }

    #[test]
    fn test_dl_err_sets_buffer() {
        with_lock(|| {
            dl_err!("error {}", 42);
            let buf = linker_get_error_buffer();
            assert_eq!(buf, "error 42");
        });
    }

    #[test]
    fn test_dl_warn_does_not_panic() {
        dl_warn!("test warn {}", 1);
    }

    #[test]
    fn test_error_restorer() {
        with_lock(|| {
            dl_err!("first error");
            assert_eq!(linker_get_error_buffer(), "first error");
            {
                let _restorer = DlErrorRestorer::new();
                dl_err!("second error");
                assert_eq!(linker_get_error_buffer(), "second error");
            }
            assert_eq!(linker_get_error_buffer(), "first error");
        });
    }

    #[test]
    fn test_linker_args_env() {
        with_lock(|| {
            linker_set_args(
                3,
                vec!["prog".into(), "-a".into(), "arg".into()],
                vec!["HOME=/tmp".into(), "PATH=/usr/bin".into()],
            );
            assert_eq!(linker_argc(), 3);
            assert_eq!(linker_argv()[0], "prog");
            assert_eq!(linker_argv()[1], "-a");
            assert_eq!(linker_envp()[0], "HOME=/tmp");
        });
    }

    #[test]
    fn test_dl_warn_documented_change() {
        dl_warn_documented_change!(29, "docs/foo.md", "test message");
    }

    #[test]
    fn test_error_buffer_size() {
        assert_eq!(linker_get_error_buffer_size(), 768);
    }

    #[test]
    fn test_error_buffer_cleared_on_set() {
        with_lock(|| {
            dl_err!("first");
            dl_err!("second");
            assert_eq!(linker_get_error_buffer(), "second");
        });
    }

    #[test]
    fn test_linker_args_defaults() {
        with_lock(|| {
            assert_eq!(linker_argc(), 0);
            assert!(linker_argv().is_empty());
            assert!(linker_envp().is_empty());
        });
    }
}

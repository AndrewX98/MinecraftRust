use std::ffi::{c_char, CStr, CString};
use std::sync::atomic::{AtomicI32, Ordering};
use std::sync::Mutex;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Version {
    pub major: i32,
    pub minor: i32,
    pub patch: i32,
    pub revision: i32,
    pub code: i32,
}
impl Version {
    pub fn get_string(&self) -> String {
        format!("{}.{}.{}.{}", self.major, self.minor, self.patch, self.revision)
    }
}
pub fn decode(version_code: i64) -> Option<(i32, i32, i32, i32)> {
    let is_android = version_code >= 950_000_000 && version_code < 990_000_000;
    let is_chromeos = version_code >= 1_950_000_000 && version_code < 1_990_000_000;
    if !(is_android || is_chromeos) {
        return None;
    }
    let mut parts = (version_code % 10_000_000) as i32;
    let major = 1;
    let minor = parts / 100_000;
    parts = parts % 100_000;
    let patch = parts / 100;
    parts = parts % 100;
    let revision = parts;
    Some((major, minor, patch, revision))
}

/// Decoded fields exposed as stable addresses for the `getApi` package_*
/// intrinsics. Written once by `mc_init_version`; read by mods through the
/// addresses registered in `libmcpelauncher_mod.so`. `AtomicI32` is
/// `repr(transparent)` over `i32`, so `as_ptr()` is a valid `*const i32`.
static MAJOR: AtomicI32 = AtomicI32::new(0);
static MINOR: AtomicI32 = AtomicI32::new(0);
static PATCH: AtomicI32 = AtomicI32::new(0);
static REVISION: AtomicI32 = AtomicI32::new(0);
static CODE: AtomicI32 = AtomicI32::new(0);

/// Leaked package CString pointer (never freed after write), so the address
/// registered under `mcpelauncher_package_name` stays valid for mods even if
/// `mc_init_version` runs again. `PkgPtr` exists because a raw pointer isn't
/// `Send`; access is single-threaded (boot writes once before getApi reads).
struct PkgPtr(*const c_char);
unsafe impl Send for PkgPtr {}
unsafe impl Sync for PkgPtr {}
static PACKAGE: Mutex<PkgPtr> = Mutex::new(PkgPtr(std::ptr::null()));

pub fn code_addr() -> *const i32 {
    CODE.as_ptr()
}
pub fn major_addr() -> *const i32 {
    MAJOR.as_ptr()
}
pub fn minor_addr() -> *const i32 {
    MINOR.as_ptr()
}
pub fn patch_addr() -> *const i32 {
    PATCH.as_ptr()
}
pub fn revision_addr() -> *const i32 {
    REVISION.as_ptr()
}
pub fn package_cstr() -> *const c_char {
    let mut g = PACKAGE.lock().unwrap();
    if g.0.is_null() {
        // C++ `MinecraftVersion::package.c_str()` is never null even when unset.
        let leaked: &'static CString = Box::leak(Box::new(CString::new("").unwrap()));
        g.0 = leaked.as_ptr();
    }
    g.0
}

static STATE: Mutex<Version> = Mutex::new(Version {
    major: 0,
    minor: 0,
    patch: 0,
    revision: 0,
    code: 0,
});
#[no_mangle]
pub extern "C" fn mc_init_version(package: *const c_char, version_code: i32) {
    let package = unsafe {
        if package.is_null() {
            None
        } else {
            Some(std::ffi::CStr::from_ptr(package).to_string_lossy().into_owned())
        }
    };
    let (major, minor, patch, revision) = match decode(version_code as i64) {
        Some(v) => v,
        None => (0, 0, 0, 0),
    };
    {
        let mut state = STATE.lock().unwrap();
        state.major = major;
        state.minor = minor;
        state.patch = patch;
        state.revision = revision;
        state.code = version_code;
    }
    // Write the address-stable mirrors used by getApi.
    MAJOR.store(major, Ordering::Relaxed);
    MINOR.store(minor, Ordering::Relaxed);
    PATCH.store(patch, Ordering::Relaxed);
    REVISION.store(revision, Ordering::Relaxed);
    CODE.store(version_code, Ordering::Relaxed);
    let pkg = match package {
        Some(s) => s,
        None => String::new(),
    };
    let leaked: &'static CString = Box::leak(Box::new(
        CString::new(pkg).unwrap_or_else(|_| CString::new("").unwrap()),
    ));
    PACKAGE.lock().unwrap().0 = leaked.as_ptr();
}

/// `#[no_mangle]` twin of `MinecraftUtils::workaroundLocaleBug`
/// (`minecraft_utils.cpp:62`): force a locale MCPE's bundled libc++ recognizes.
#[no_mangle]
pub extern "C" fn mc_workaround_locale_bug() {
    // setenv("LC_ALL", "C", 1)
    let k = CString::new("LC_ALL").unwrap();
    let v = CString::new("C").unwrap();
    unsafe {
        libc::setenv(k.as_ptr(), v.as_ptr(), 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decode_android() {
        assert_eq!(decode(962_112_004), Some((1, 21, 120, 4)));
    }
    #[test]
    fn decode_chromeos() {
        assert_eq!(decode(1_987_654_321), Some((1, 76, 543, 21)));
    }
    #[test]
    fn decode_non_android_passthrough() {
        assert_eq!(decode(123), None);
        assert_eq!(decode(949_999_999), None);
        assert_eq!(decode(990_000_000), None);
    }
    #[test]
    fn get_string_android() {
        let (maj, min, pat, rev) = decode(962_112_004).unwrap();
        let v = Version { major: maj, minor: min, patch: pat, revision: rev, code: 962_112_004 };
        assert_eq!(v.get_string(), "1.21.120.4");
    }
    #[test]
    fn get_string_non_android() {
        let v = Version { major: 0, minor: 0, patch: 0, revision: 0, code: 123 };
        assert_eq!(v.get_string(), "0.0.0.0");
    }
    #[test]
    fn atomic_mirrors_follow_init() {
        mc_init_version(c"com.mojang.minecraftpe".as_ptr(), 962_112_004);
        assert_eq!(unsafe { *code_addr() }, 962_112_004);
        assert_eq!(unsafe { *major_addr() }, 1);
        assert_eq!(unsafe { *minor_addr() }, 21);
        assert_eq!(unsafe { *patch_addr() }, 120);
        assert_eq!(unsafe { *revision_addr() }, 4);
        let p = package_cstr();
        assert_eq!(unsafe { CStr::from_ptr(p) }.to_str().unwrap(), "com.mojang.minecraftpe");
    }
}
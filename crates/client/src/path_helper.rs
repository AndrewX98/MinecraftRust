//! Rust port of `mcpelauncher-common`'s `PathHelper` (path_helper.cpp).
//!
//! Owns the `PathInfo` state (game/data/cache dir resolution, XDG defaults and
//! `-dg`/`-dd`/`-dc` CLI overrides) and exports a C FFI surface
//! (`path_helper_*`) that the remaining C++ code calls instead of the deleted
//! C++ `PathHelper` class.

use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use std::sync::Mutex;

const APP_DIR_NAME: &str = "mcpelauncher";

// Mirrors the C++ `DEV_EXTRA_PATHS` compile-time define (runtime files live at
// the workspace root, two levels up from the client crate manifest dir).
const DEV_EXTRA_PATHS: &[&str] = &[
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime"),
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../runtime/gamecontrollerdb"),
];

struct PathInfo {
    app_dir: String,
    home_dir: String,
    data_home: String,
    data_dirs: Vec<String>,
    cache_home: String,
    override_data_dir: String,
    override_cache_dir: String,
    game_dir: String,
}

impl PathInfo {
    fn new() -> Self {
        let app_dir = Self::find_app_dir();
        let home_dir = Self::find_user_home();
        let cwd = get_working_dir();
        // Mirror the C++ PathInfo ctor: if libminecraftpe.so sits next to the
        // working directory, treat the working directory as the data dir.
        let cwd_lib = format!("{}lib/{}/libminecraftpe.so", cwd, get_abi_dir());
        if Path::new(&cwd_lib).exists() {
            return PathInfo {
                app_dir,
                home_dir,
                data_home: String::new(),
                data_dirs: Vec::new(),
                cache_home: String::new(),
                override_data_dir: cwd,
                override_cache_dir: String::new(),
                game_dir: String::new(),
            };
        }
        let data_home = Self::xdg_data_home(&home_dir);
        let data_dirs = Self::xdg_data_dirs();
        let cache_home = Self::xdg_cache_home(&home_dir);
        PathInfo {
            app_dir,
            home_dir,
            data_home,
            data_dirs,
            cache_home,
            override_data_dir: String::new(),
            override_cache_dir: String::new(),
            game_dir: String::new(),
        }
    }

    fn find_app_dir() -> String {
        let buf = std::fs::read_link("/proc/self/exe").unwrap_or_default();
        if let Some(parent) = buf.parent() {
            parent.to_string_lossy().to_string()
        } else {
            String::new()
        }
    }

    fn find_user_home() -> String {
        std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"))
    }

    fn xdg_data_home(home_dir: &str) -> String {
        std::env::var("XDG_DATA_HOME")
            .unwrap_or_else(|_| format!("{}/.local/share", home_dir))
    }

    fn xdg_data_dirs() -> Vec<String> {
        let dirs = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
        if dirs.is_empty() {
            vec!["/usr/local/share/".into(), "/usr/share/".into()]
        } else {
            dirs.split(':').map(|s| s.to_string()).collect()
        }
    }

    fn xdg_cache_home(home_dir: &str) -> String {
        std::env::var("XDG_CACHE_HOME")
            .unwrap_or_else(|_| format!("{}/.cache", home_dir))
    }
}

static PATH_INFO: std::sync::LazyLock<Mutex<PathInfo>> =
    std::sync::LazyLock::new(|| Mutex::new(PathInfo::new()));

pub fn get_app_dir() -> String {
    PATH_INFO.lock().unwrap().app_dir.clone()
}

pub fn get_primary_data_directory() -> String {
    let info = PATH_INFO.lock().unwrap();
    if !info.override_data_dir.is_empty() {
        info.override_data_dir.clone()
    } else {
        format!("{}/{}/", info.data_home, APP_DIR_NAME)
    }
}

pub fn get_cache_directory() -> String {
    let info = PATH_INFO.lock().unwrap();
    if !info.override_cache_dir.is_empty() {
        info.override_cache_dir.clone()
    } else {
        format!("{}/{}/", info.cache_home, APP_DIR_NAME)
    }
}

pub fn get_game_dir() -> String {
    let info = PATH_INFO.lock().unwrap();
    if !info.game_dir.is_empty() {
        info.game_dir.clone()
    } else {
        get_primary_data_directory()
    }
}

fn normalize_dir(dir: &str) -> String {
    if !dir.is_empty() && !dir.ends_with('/') {
        format!("{}/", dir)
    } else {
        dir.to_string()
    }
}

pub fn set_game_dir(dir: &str) {
    let mut info = PATH_INFO.lock().unwrap();
    info.game_dir = normalize_dir(dir);
}

pub fn set_data_dir(dir: &str) {
    let mut info = PATH_INFO.lock().unwrap();
    info.override_data_dir = normalize_dir(dir);
}

pub fn set_cache_dir(dir: &str) {
    let mut info = PATH_INFO.lock().unwrap();
    info.override_cache_dir = normalize_dir(dir);
}

pub fn file_exists(path: &str) -> bool {
    Path::new(path).exists()
}

pub fn get_parent_dir(path: &str) -> String {
    Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn get_working_dir() -> String {
    std::env::current_dir()
        .map(|p| format!("{}/", p.display()))
        .unwrap_or_default()
}

fn candidate_data_dirs(info: &PathInfo, path: &str) -> Vec<String> {
    let mut v = Vec::new();
    if !info.override_data_dir.is_empty() {
        v.push(format!("{}{}", info.override_data_dir, path));
    } else {
        v.push(format!("{}/{}", info.app_dir, path));
        v.push(format!("{}/{}/{}", info.data_home, APP_DIR_NAME, path));
    }
    for dir in DEV_EXTRA_PATHS {
        v.push(format!("{}/{}", dir, path));
    }
    v
}

pub fn find_data_file(path: &str) -> Option<String> {
    let info = PATH_INFO.lock().unwrap();
    for p in candidate_data_dirs(&info, path) {
        if file_exists(&p) {
            return Some(p);
        }
    }
    for dir in &info.data_dirs {
        let p = format!("{}/{}/{}", dir.trim_end_matches('/'), APP_DIR_NAME, path);
        if file_exists(&p) {
            return Some(p);
        }
    }
    let p = format!(
        "{}/share/mcpelauncher/{}",
        get_parent_dir(&info.app_dir),
        path
    );
    if file_exists(&p) {
        return Some(p);
    }
    None
}

pub fn find_game_file(path: &str) -> Option<String> {
    {
        let info = PATH_INFO.lock().unwrap();
        if !info.game_dir.is_empty() {
            return Some(format!("{}{}", info.game_dir, path));
        }
    }
    find_data_file(path)
}

pub fn find_all_data_files(path: &str) -> Vec<String> {
    let info = PATH_INFO.lock().unwrap();
    let mut results = Vec::new();
    for p in candidate_data_dirs(&info, path) {
        if file_exists(&p) {
            results.push(p);
        }
    }
    for dir in &info.data_dirs {
        let p = format!("{}/{}/{}", dir.trim_end_matches('/'), APP_DIR_NAME, path);
        if file_exists(&p) {
            results.push(p);
        }
    }
    let p = format!(
        "{}/share/mcpelauncher/{}",
        get_parent_dir(&info.app_dir),
        path
    );
    if file_exists(&p) {
        results.push(p);
    }
    results
}

pub fn get_abi_dir() -> &'static str {
    if cfg!(target_arch = "x86_64") {
        "x86_64"
    } else if cfg!(target_arch = "x86") {
        "x86"
    } else if cfg!(target_arch = "aarch64") {
        "arm64-v8a"
    } else if cfg!(target_arch = "arm") {
        "armeabi-v7a"
    } else {
        "unsupported"
    }
}

// ===========================================================================
// C FFI surface (replaces the C++ PathHelper symbols; consumed by capi.cpp,
// jni_support.cpp, minecraft_utils.cpp, window_callbacks_stub.cpp, etc.)
// ===========================================================================

/// Return a pointer valid until the next call on the same thread. Mirrors the
/// old C++ `static std::string` pattern: callers copy the string immediately.
fn cstr_static(s: String) -> *const c_char {
    thread_local! {
        static CACHE: std::cell::RefCell<Vec<CString>> =
            const { std::cell::RefCell::new(Vec::new()) };
    }
    CACHE.with(|cache| {
        let mut v = cache.borrow_mut();
        v.push(CString::new(s).unwrap_or_else(|_| CString::new("").unwrap()));
        v.last().unwrap().as_ptr()
    })
}

unsafe fn cstr_to_string(p: *const c_char) -> Option<String> {
    if p.is_null() {
        None
    } else {
        Some(CStr::from_ptr(p).to_string_lossy().into_owned())
    }
}

#[no_mangle]
pub extern "C" fn path_helper_get_app_dir() -> *const c_char {
    cstr_static(get_app_dir())
}

#[no_mangle]
pub extern "C" fn path_helper_get_primary_data_directory() -> *const c_char {
    cstr_static(get_primary_data_directory())
}

#[no_mangle]
pub extern "C" fn path_helper_get_cache_directory() -> *const c_char {
    cstr_static(get_cache_directory())
}

#[no_mangle]
pub extern "C" fn path_helper_get_game_dir() -> *const c_char {
    cstr_static(get_game_dir())
}

#[no_mangle]
pub extern "C" fn path_helper_get_abi_dir() -> *const c_char {
    cstr_static(get_abi_dir().to_string())
}

#[no_mangle]
pub unsafe extern "C" fn path_helper_set_game_dir(dir: *const c_char) {
    if let Some(s) = cstr_to_string(dir) {
        set_game_dir(&s);
    }
}

#[no_mangle]
pub unsafe extern "C" fn path_helper_set_data_dir(dir: *const c_char) {
    if let Some(s) = cstr_to_string(dir) {
        set_data_dir(&s);
    }
}

#[no_mangle]
pub unsafe extern "C" fn path_helper_set_cache_dir(dir: *const c_char) {
    if let Some(s) = cstr_to_string(dir) {
        set_cache_dir(&s);
    }
}

#[no_mangle]
pub unsafe extern "C" fn path_helper_find_data_file(path: *const c_char) -> *const c_char {
    match cstr_to_string(path).and_then(|s| find_data_file(&s)) {
        Some(p) => cstr_static(p),
        None => std::ptr::null(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn path_helper_find_game_file(path: *const c_char) -> *const c_char {
    match cstr_to_string(path).and_then(|s| find_game_file(&s)) {
        Some(p) => cstr_static(p),
        None => std::ptr::null(),
    }
}

/// Invoke `cb` for every existing candidate of `path`. Mirrors the C++
/// `PathHelper::findAllDataFiles(std::string, std::function<void(std::string)>)`.
#[no_mangle]
pub unsafe extern "C" fn path_helper_find_all_data_files(
    path: *const c_char,
    cb: Option<unsafe extern "C" fn(*const c_char, *mut c_void)>,
    user: *mut c_void,
) {
    let cb = match cb {
        Some(cb) => cb,
        None => return,
    };
    let Some(path) = cstr_to_string(path) else { return };
    for p in find_all_data_files(&path) {
        cb(cstr_static(p), user);
    }
}

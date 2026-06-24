use std::path::Path;
use std::sync::Mutex;

const APP_DIR_NAME: &str = "mcpelauncher";

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
        std::env::var("HOME").unwrap_or_else(|_| {
            let buf = unsafe { libc::getpwuid_r(libc::getuid()) };
            // fallback
            String::from("/tmp")
        })
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

pub fn set_game_dir(dir: &str) {
    let mut info = PATH_INFO.lock().unwrap();
    let dir = if !dir.is_empty() && !dir.ends_with('/') {
        format!("{}/", dir)
    } else {
        dir.to_string()
    };
    info.game_dir = dir;
}

pub fn set_data_dir(dir: &str) {
    let mut info = PATH_INFO.lock().unwrap();
    let dir = if !dir.is_empty() && !dir.ends_with('/') {
        format!("{}/", dir)
    } else {
        dir.to_string()
    };
    info.override_data_dir = dir;
}

pub fn set_cache_dir(dir: &str) {
    let mut info = PATH_INFO.lock().unwrap();
    let dir = if !dir.is_empty() && !dir.ends_with('/') {
        format!("{}/", dir)
    } else {
        dir.to_string()
    };
    info.override_cache_dir = dir;
}

pub fn file_exists(path: &str) -> bool {
    Path::new(path).exists()
}

pub fn get_parent_dir(path: &str) -> String {
    Path::new(path).parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn get_working_dir() -> String {
    std::env::current_dir()
        .map(|p| format!("{}/", p.display()))
        .unwrap_or_default()
}

pub fn find_data_file(path: &str) -> Option<String> {
    let info = PATH_INFO.lock().unwrap();
    let candidates: Vec<String> = if !info.override_data_dir.is_empty() {
        vec![format!("{}{}", info.override_data_dir, path)]
    } else {
        let mut v = Vec::new();
        v.push(format!("{}/{}", info.app_dir, path));
        v.push(format!("{}/{}/{}", info.data_home, APP_DIR_NAME, path));
        v
    };
    for p in &candidates {
        if Path::new(p).exists() {
            return Some(p.clone());
        }
    }
    for dir in &info.data_dirs {
        let p = format!("{}/{}/{}", dir, APP_DIR_NAME, path);
        if Path::new(&p).exists() {
            return Some(p);
        }
    }
    None
}

pub fn find_game_file(path: &str) -> Option<String> {
    let info = PATH_INFO.lock().unwrap();
    if !info.game_dir.is_empty() {
        Some(format!("{}{}", info.game_dir, path))
    } else {
        find_data_file(path)
    }
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

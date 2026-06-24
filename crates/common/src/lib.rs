use std::path::Path;
use std::sync::{LazyLock, Mutex};

const APP_DIR_NAME: &str = "mcpelauncher";

struct PathInfo {
    app_dir: String,
    #[allow(dead_code)]
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
        let app_dir = find_app_dir();
        let home_dir = find_user_home();
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        let cwd_lib = format!("{}/lib/{}/libminecraftpe.so", cwd, get_abi_dir());

        let (override_data_dir, data_home, data_dirs, cache_home) = if Path::new(&cwd_lib).exists() {
            (cwd, String::new(), vec![], String::new())
        } else {
            let data_home = std::env::var("XDG_DATA_HOME")
                .unwrap_or_else(|_| format!("{}/.local/share", home_dir));
            let data_dirs_str = std::env::var("XDG_DATA_DIRS").unwrap_or_default();
            let data_dirs: Vec<String> = if data_dirs_str.is_empty() {
                vec![
                    "/usr/local/share/".into(),
                    "/usr/share/".into(),
                ]
            } else {
                data_dirs_str.split(':').map(|s| s.to_string()).collect()
            };
            let cache_home = std::env::var("XDG_CACHE_HOME")
                .unwrap_or_else(|_| format!("{}/.cache", home_dir));
            (String::new(), data_home, data_dirs, cache_home)
        };

        PathInfo {
            app_dir,
            home_dir,
            data_home,
            data_dirs,
            cache_home,
            override_data_dir,
            override_cache_dir: String::new(),
            game_dir: String::new(),
        }
    }
}

static PATH_INFO: LazyLock<Mutex<PathInfo>> = LazyLock::new(|| Mutex::new(PathInfo::new()));

fn find_app_dir() -> String {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_default()
}

fn find_user_home() -> String {
    std::env::var("HOME").unwrap_or_else(|_| String::new())
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

pub struct PathHelper;

impl PathHelper {
    pub fn file_exists(path: &str) -> bool {
        Path::new(path).exists()
    }

    pub fn get_parent_dir(path: &str) -> String {
        let trimmed = path.trim_end_matches('/');
        match trimmed.rfind('/') {
            Some(pos) if pos > 0 => trimmed[..pos].to_string(),
            Some(_) => String::new(),
            None => String::new(),
        }
    }

    pub fn get_working_dir() -> String {
        std::env::current_dir()
            .map(|p| format!("{}/", p.to_string_lossy()))
            .unwrap_or_default()
    }

    pub fn get_primary_data_directory() -> String {
        let info = PATH_INFO.lock().unwrap();
        if !info.override_data_dir.is_empty() {
            return info.override_data_dir.clone();
        }
        format!("{}/{}/", info.data_home, APP_DIR_NAME)
    }

    pub fn get_cache_directory() -> String {
        let info = PATH_INFO.lock().unwrap();
        if !info.override_cache_dir.is_empty() {
            return info.override_cache_dir.clone();
        }
        format!("{}/{}/", info.cache_home, APP_DIR_NAME)
    }

    pub fn get_game_dir() -> String {
        let info = PATH_INFO.lock().unwrap();
        if !info.game_dir.is_empty() {
            return info.game_dir.clone();
        }
        Self::get_primary_data_directory()
    }

    pub fn set_game_dir(game_dir: &str) {
        let mut info = PATH_INFO.lock().unwrap();
        let dir = if !game_dir.is_empty() && !game_dir.ends_with('/') {
            format!("{}/", game_dir)
        } else {
            game_dir.to_string()
        };
        info.game_dir = dir;
    }

    pub fn set_data_dir(data_dir: &str) {
        let mut info = PATH_INFO.lock().unwrap();
        let dir = if !data_dir.is_empty() && !data_dir.ends_with('/') {
            format!("{}/", data_dir)
        } else {
            data_dir.to_string()
        };
        info.override_data_dir = dir;
    }

    pub fn set_cache_dir(cache_dir: &str) {
        let mut info = PATH_INFO.lock().unwrap();
        let dir = if !cache_dir.is_empty() && !cache_dir.ends_with('/') {
            format!("{}/", cache_dir)
        } else {
            cache_dir.to_string()
        };
        info.override_cache_dir = dir;
    }

    pub fn find_game_file(path: &str) -> String {
        let info = PATH_INFO.lock().unwrap();
        if !info.game_dir.is_empty() {
            return format!("{}{}", info.game_dir, path);
        }
        drop(info);
        Self::find_data_file(path)
    }

    pub fn get_icon_path() -> String {
        Self::find_game_file("assets/icon.png")
    }

    pub fn get_app_dir() -> String {
        PATH_INFO.lock().unwrap().app_dir.clone()
    }

    pub fn find_data_file(path: &str) -> String {
        let info = PATH_INFO.lock().unwrap();

        if !info.override_data_dir.is_empty() {
            let p = format!("{}{}", info.override_data_dir, path);
            if Path::new(&p).exists() {
                return p;
            }
        } else {
            let p = format!("{}/{}", info.app_dir, path);
            if Path::new(&p).exists() {
                return p;
            }
            let p = format!("{}/{}/{}", info.data_home, APP_DIR_NAME, path);
            if Path::new(&p).exists() {
                return p;
            }
        }

        for dir in &info.data_dirs {
            let p = format!("{}/{}/{}", dir.trim_end_matches('/'), APP_DIR_NAME, path);
            if Path::new(&p).exists() {
                return p;
            }
        }

        let p = format!(
            "{}/share/mcpelauncher/{}",
            Self::get_parent_dir(&info.app_dir),
            path
        );
        if Path::new(&p).exists() {
            return p;
        }

        panic!("Failed to find data file: {}", path);
    }

    pub fn find_all_data_files(path: &str) -> Vec<String> {
        let info = PATH_INFO.lock().unwrap();
        let mut results = Vec::new();

        if !info.override_data_dir.is_empty() {
            let p = format!("{}{}", info.override_data_dir, path);
            if Path::new(&p).exists() {
                results.push(p);
            }
        } else {
            let p = format!("{}/{}", info.app_dir, path);
            if Path::new(&p).exists() {
                results.push(p);
            }
            let p = format!("{}/{}/{}", info.data_home, APP_DIR_NAME, path);
            if Path::new(&p).exists() {
                results.push(p);
            }
        }

        for dir in &info.data_dirs {
            let p = format!("{}/{}/{}", dir.trim_end_matches('/'), APP_DIR_NAME, path);
            if Path::new(&p).exists() {
                results.push(p);
            }
        }

        let p = format!(
            "{}/share/mcpelauncher/{}",
            Self::get_parent_dir(&info.app_dir),
            path
        );
        if Path::new(&p).exists() {
            results.push(p);
        }

        results
    }
}

use std::fs;
use std::path::Path;

pub struct FileUtil;

impl FileUtil {
    pub fn get_parent(path: &str) -> String {
        let path = path.trim_end_matches('/');
        match path.rfind('/') {
            Some(pos) if pos > 0 => path[..pos].to_string(),
            Some(_) => String::new(),
            None => String::new(),
        }
    }

    pub fn exists(path: &str) -> bool {
        Path::new(path).exists()
    }

    pub fn is_directory(path: &str) -> bool {
        Path::new(path).is_dir()
    }

    pub fn mkdir_recursive(path: &str) -> std::io::Result<()> {
        fs::create_dir_all(path)
    }

    pub fn read_file(path: &str) -> std::io::Result<String> {
        fs::read_to_string(path)
    }

    pub fn read_file_bytes(path: &str) -> std::io::Result<Vec<u8>> {
        fs::read(path)
    }
}

pub struct EnvPathUtil;

impl EnvPathUtil {
    pub fn get_app_dir() -> String {
        let exe = std::env::current_exe().ok().unwrap_or_default();
        exe.parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub fn get_working_dir() -> String {
        std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub fn get_home_dir() -> String {
        dirs::home_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default()
    }

    pub fn get_data_home() -> String {
        if let Ok(env) = std::env::var("XDG_DATA_HOME") {
            return env;
        }
        Self::get_home_dir() + "/.local/share"
    }

    pub fn get_cache_home() -> String {
        if let Ok(env) = std::env::var("XDG_CACHE_HOME") {
            return env;
        }
        Self::get_home_dir() + "/.cache"
    }

    pub fn find_in_path(what: &str) -> Option<String> {
        let path = std::env::var("PATH").unwrap_or_else(|_| "/bin:/usr/bin".into());
        Self::find_in_path_with(what, &path, None)
    }

    pub fn find_in_path_with(
        what: &str,
        path: &str,
        cwd: Option<&str>,
    ) -> Option<String> {
        for dir in path.split(':') {
            let dir = if dir.is_empty() {
                cwd.unwrap_or(".")
            } else {
                dir
            };
            let full_path = if dir.ends_with('/') {
                format!("{}{}", dir, what)
            } else {
                format!("{}/{}", dir, what)
            };
            if Path::new(&full_path).is_file() {
                return Some(full_path);
            }
        }
        None
    }
}

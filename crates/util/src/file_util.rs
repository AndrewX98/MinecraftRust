use std::fs;
use std::path::Path;

pub struct FileUtil;

impl FileUtil {
    // Mirrors the C++ FileUtil::getParent exactly, including the
    // trailing-slash recursion and skipping of consecutive slashes.
    pub fn get_parent(path: &str) -> String {
        let bytes = path.as_bytes();
        match path.rfind('/') {
            Some(mut iof) => {
                let ends_with_slash = iof == path.len() - 1;
                while iof > 0 && bytes[iof - 1] == b'/' {
                    iof -= 1;
                }
                let ret = path[..iof].to_string();
                if ends_with_slash {
                    Self::get_parent(&ret)
                } else {
                    ret
                }
            }
            None => String::new(),
        }
    }

    pub fn exists(path: &str) -> bool {
        Path::new(path).exists()
    }

    pub fn is_directory(path: &str) -> bool {
        Path::new(path).is_dir()
    }

    // C++ throws if the path exists as a file; create_dir_all errors on that
    // case too, so the behavior matches for consumers.
    pub fn mkdir_recursive(path: &str) -> std::io::Result<()> {
        fs::create_dir_all(path)
    }

    // Matches C++ FileUtil::readFile (open O_RDONLY, fail on dirs/missing).
    pub fn read_file_bytes(path: &str) -> std::io::Result<Vec<u8>> {
        fs::read(path)
    }

    pub fn read_file(path: &str) -> std::io::Result<String> {
        fs::read_to_string(path)
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
        // Mirrors C++: empty PATH falls back to the default.
        let path = match std::env::var("PATH") {
            Ok(p) if !p.is_empty() => p,
            _ => "/bin:/usr/bin".into(),
        };
        Self::find_in_path_with(what, &path, None)
    }

    // Mirrors the C++ EnvPathUtil::findInPath exactly: empty segments and
    // relative segments are prefixed with `cwd`, an empty segment with no cwd
    // becomes ".", and candidates must be executable (access X_OK).
    pub fn find_in_path_with(
        what: &str,
        path: &str,
        cwd: Option<&str>,
    ) -> Option<String> {
        let cwd = cwd.unwrap_or("");
        let cwd_len = cwd.len();
        let will_append_slash_cwd = cwd_len > 0 && !cwd.ends_with('/');
        for seg in path.split(':') {
            let len = seg.len();
            let will_prefix_with_cwd = len == 0 || !seg.starts_with('/');
            let will_replace_with_dot = len == 0 && cwd_len == 0;
            let will_append_slash = will_replace_with_dot || (len != 0 && !seg.ends_with('/'));
            let mut buf = String::new();
            if will_prefix_with_cwd {
                buf.push_str(cwd);
                if will_append_slash_cwd {
                    buf.push('/');
                }
            }
            if will_replace_with_dot {
                buf.push('.');
            } else {
                buf.push_str(seg);
            }
            if will_append_slash {
                buf.push('/');
            }
            buf.push_str(what);
            if is_executable(&buf) {
                return Some(buf);
            }
        }
        None
    }
}

fn is_executable(path: &str) -> bool {
    match std::ffi::CString::new(path) {
        Ok(c) => unsafe { libc::access(c.as_ptr(), libc::X_OK) == 0 },
        Err(_) => false,
    }
}

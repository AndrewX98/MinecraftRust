use std::ffi::CString;

pub const K_ZIP_FILE_SEPARATOR: &str = "!/";

pub fn format_string(str: &mut String, params: &[(String, String)]) {
    let mut pos = 0;
    while pos < str.len() {
        match str[pos..].find('$') {
            None => break,
            Some(rel) => {
                pos += rel;
                let mut matched = false;
                for (token, replacement) in params {
                    if pos + 1 + token.len() <= str.len()
                        && str[pos + 1..pos + 1 + token.len()] == **token
                    {
                        str.replace_range(pos..pos + 1 + token.len(), replacement);
                        pos += replacement.len();
                        matched = true;
                        break;
                    } else if pos + 3 + token.len() <= str.len()
                        && &str[pos + 1..pos + 2] == "{"
                        && &str[pos + 2..pos + 2 + token.len()] == token.as_str()
                        && &str[pos + 2 + token.len()..pos + 3 + token.len()] == "}"
                    {
                        let full_token_len = token.len() + 3;
                        str.replace_range(pos..pos + full_token_len, replacement);
                        pos += replacement.len();
                        matched = true;
                        break;
                    }
                }
                if !matched {
                    pos += 1;
                }
            }
        }
    }
}

pub fn dirname(path: &str) -> String {
    let path = if path.is_empty() { "." } else { path };
    match path.rfind('/') {
        Some(0) => "/".to_string(),
        Some(i) => path[..i].to_string(),
        None => ".".to_string(),
    }
}

pub fn basename(path: &str) -> &str {
    let path = if path.is_empty() { "." } else { path };
    match path.rfind('/') {
        Some(i) => &path[i + 1..],
        None => path,
    }
}

pub fn normalize_path(path: &str) -> Option<String> {
    if !path.starts_with('/') {
        log::debug!("normalize_path - invalid input: \"{}\", the input path should be absolute", path);
        return None;
    }

    let mut buf = Vec::with_capacity(path.len() + 1);
    let bytes = path.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'/' {
            if i + 1 < bytes.len() {
                let c1 = bytes[i + 1];
                if c1 == b'.' {
                    if i + 2 < bytes.len() && bytes[i + 2] == b'/' {
                        i += 2;
                        continue;
                    } else if c1 == b'.' && bytes[i + 2] == b'.'
                        && (i + 3 >= bytes.len() || bytes[i + 3] == b'/')
                    {
                        i += 3;
                        while buf.last().copied() != Some(b'/') && !buf.is_empty() {
                            buf.pop();
                        }
                        buf.pop();
                        if i >= bytes.len() {
                            buf.push(b'/');
                        }
                        continue;
                    }
                } else if c1 == b'/' {
                    i += 1;
                    continue;
                }
            }
        }
        buf.push(bytes[i]);
        i += 1;
    }

    Some(unsafe { String::from_utf8_unchecked(buf) })
}

pub fn file_is_in_dir(file: &str, dir: &str) -> bool {
    if let Some(rest) = file.strip_prefix(dir) {
        rest.starts_with('/') && rest[1..].find('/').is_none()
    } else {
        false
    }
}

pub fn file_is_under_dir(file: &str, dir: &str) -> bool {
    if let Some(rest) = file.strip_prefix(dir) {
        rest.starts_with('/')
    } else {
        false
    }
}

pub fn parse_zip_path(input_path: &str) -> Option<(String, String)> {
    let normalized = normalize_path(input_path)?;
    log::trace!("Trying zip file open from path \"{}\" -> normalized \"{}\"", input_path, normalized);

    let separator = normalized.find(K_ZIP_FILE_SEPARATOR)?;
    let zip_path = normalized[..separator].to_string();
    let entry_path = normalized[separator + K_ZIP_FILE_SEPARATOR.len()..].to_string();
    Some((zip_path, entry_path))
}

pub fn page_start(offset: i64) -> i64 {
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    offset & !(page_size - 1)
}

pub fn page_offset(offset: i64) -> usize {
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (offset & (page_size - 1)) as usize
}

pub fn safe_add(a: i64, b: usize) -> Option<i64> {
    if a < 0 {
        return None;
    }
    let result = a.checked_add(b as i64)?;
    Some(result)
}

pub fn split_path(path: &str, delimiters: &str) -> Vec<String> {
    if path.is_empty() {
        return Vec::new();
    }
    path.split(|c| delimiters.contains(c))
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

pub fn resolve_paths(paths: &[String]) -> Vec<String> {
    let mut resolved = Vec::new();
    for path in paths {
        if path.is_empty() {
            continue;
        }
        let r = resolve_path(path);
        if !r.is_empty() {
            resolved.push(r);
        }
    }
    resolved
}

pub fn resolve_path(path: &str) -> String {
    let cpath = CString::new(path.as_bytes()).unwrap();
    let mut resolved_buf = vec![0u8; libc::PATH_MAX as usize];
    let ret = unsafe {
        libc::realpath(cpath.as_ptr(), resolved_buf.as_mut_ptr() as *mut libc::c_char)
    };

    if !ret.is_null() {
        let resolved_path = unsafe {
            std::ffi::CStr::from_ptr(resolved_buf.as_ptr() as *const libc::c_char)
                .to_str()
                .unwrap()
                .to_string()
        };
        let cpath = CString::new(resolved_path.as_bytes()).unwrap();
        unsafe {
            let mut stat_buf: libc::stat = std::mem::zeroed();
            if libc::stat(cpath.as_ptr(), &mut stat_buf) == -1 {
                log::warn!("Warning: cannot stat file \"{}\": (ignoring)", resolved_path);
                return String::new();
            }
            if stat_buf.st_mode & libc::S_IFMT != libc::S_IFDIR {
                log::warn!("Warning: \"{}\" is not a directory (ignoring)", resolved_path);
                return String::new();
            }
        }
        return resolved_path;
    }

    if let Some(normalized) = normalize_path(path) {
        if let Some((zip_path, entry_path)) = parse_zip_path(&normalized) {
            let cpath = CString::new(zip_path.as_bytes()).unwrap();
            let mut resolved_buf = vec![0u8; libc::PATH_MAX as usize];
            let ret = unsafe {
                libc::realpath(cpath.as_ptr(), resolved_buf.as_mut_ptr() as *mut libc::c_char)
            };
            if !ret.is_null() {
                let resolved_zip = unsafe {
                    std::ffi::CStr::from_ptr(resolved_buf.as_ptr() as *const libc::c_char)
                        .to_str()
                        .unwrap()
                };
                return format!("{}{}{}", resolved_zip, K_ZIP_FILE_SEPARATOR, entry_path);
            } else {
                log::warn!("Warning: unable to resolve \"{}\": (ignoring)", zip_path);
                return String::new();
            }
        }

        let cpath = CString::new(normalized.as_bytes()).unwrap();
        unsafe {
            let mut stat_buf: libc::stat = std::mem::zeroed();
            if libc::stat(cpath.as_ptr(), &mut stat_buf) == 0 && stat_buf.st_mode & libc::S_IFMT == libc::S_IFDIR
            {
                return normalized;
            }
        }
    }

    String::new()
}

pub fn is_first_stage_init() -> bool {
    static RET: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *RET.get_or_init(|| {
        let pid = unsafe { libc::getpid() };
        pid == 1 && unsafe { libc::access("/proc/self/exe\0".as_ptr() as *const libc::c_char, libc::F_OK) } == -1
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_string() {
        let params = vec![
            ("LIB".to_string(), "lib32".to_string()),
            ("SDKVER".to_string(), "42".to_string()),
        ];
        let mut s = "LIB$LIB${LIB${SDKVER}SDKVER$TEST$".to_string();
        format_string(&mut s, &params);
        assert_eq!(s, "LIBlib32${LIB42SDKVER$TEST$");
    }

    #[test]
    fn test_normalize_path_smoke() {
        assert_eq!(
            normalize_path("/../root///dir/.///dir2/somedir/../zipfile!/dir/dir9//..///afile"),
            Some("/root/dir/dir2/zipfile!/dir/afile".to_string())
        );

        assert_eq!(
            normalize_path("/../root///dir/.///dir2/somedir/.../zipfile!/.dir/dir9//..///afile"),
            Some("/root/dir/dir2/somedir/.../zipfile!/.dir/afile".to_string())
        );

        assert_eq!(normalize_path("/root/.."), Some("/".to_string()));
        assert_eq!(normalize_path("/root/notroot/.."), Some("/root/".to_string()));
        assert_eq!(normalize_path("/a/../../b"), Some("/b".to_string()));
        assert_eq!(normalize_path("/.."), Some("/".to_string()));

        assert_eq!(normalize_path("root///dir/.///dir2/somedir/../zipfile!/dir/dir9//..///afile"), None);
    }

    #[test]
    fn test_file_is_in_dir() {
        assert!(file_is_in_dir("/foo/bar/file", "/foo/bar"));
        assert!(!file_is_in_dir("/foo/bar/file", "/foo"));
        assert!(!file_is_in_dir("/foo/bar/file", "/bar/foo"));
        assert!(file_is_in_dir("/file", ""));
        assert!(!file_is_in_dir("/file", "/"));
    }

    #[test]
    fn test_file_is_under_dir() {
        assert!(file_is_under_dir("/foo/bar/file", "/foo/bar"));
        assert!(file_is_under_dir("/foo/bar/file", "/foo"));
        assert!(!file_is_under_dir("/foo/bar/file", "/bar/foo"));
        assert!(file_is_under_dir("/file", ""));
        assert!(file_is_under_dir("/foo/bar/file", ""));
        assert!(!file_is_under_dir("/file", "/"));
        assert!(!file_is_under_dir("/foo/bar/file", "/"));
    }

    #[test]
    fn test_parse_zip_path() {
        assert!(parse_zip_path("/not/a/zip/path/file.zip").is_none());
        assert!(parse_zip_path("/not/a/zip/path/file.zip!path/in/zip").is_none());
        let (zip_path, entry_path) = parse_zip_path("/zip/path/file.zip!/path/in/zip").unwrap();
        assert_eq!(zip_path, "/zip/path/file.zip");
        assert_eq!(entry_path, "path/in/zip");

        let (zip_path, entry_path) = parse_zip_path("/zip/path/file2.zip!/").unwrap();
        assert_eq!(zip_path, "/zip/path/file2.zip");
        assert_eq!(entry_path, "");
    }

    #[test]
    fn test_page_start() {
        assert_eq!(page_start(0x0001000), 0x0001000);
        assert_eq!(page_start(0x300222f), 0x3002000);
        assert_eq!(page_start(0x6001fff), 0x6001000);
    }

    #[test]
    fn test_page_offset() {
        assert_eq!(page_offset(0x0001000), 0x0);
        assert_eq!(page_offset(0x300222f), 0x22f);
        assert_eq!(page_offset(0x6001fff), 0xfff);
    }

    #[test]
    fn test_safe_add() {
        assert_eq!(safe_add(i64::MAX - 20, 21), None);
        assert_eq!(safe_add(i64::MAX - 42, 42), Some(i64::MAX));
        assert_eq!(safe_add(2000, 42), Some(2042));
    }

    #[test]
    fn test_dirname() {
        assert_eq!(dirname("/"), "/");
        assert_eq!(dirname("/foo"), "/");
        assert_eq!(dirname("/foo/bar"), "/foo");
        assert_eq!(dirname("relative/path"), "relative");
        assert_eq!(dirname("no_slash"), ".");
        assert_eq!(dirname(""), ".");
    }

    #[test]
    fn test_split_path() {
        let parts = split_path("/usr/lib:/usr/local/lib", ":");
        assert_eq!(parts, vec!["/usr/lib", "/usr/local/lib"]);

        let parts = split_path("", ":");
        assert!(parts.is_empty());

        let parts = split_path("a:b:c", ":");
        assert_eq!(parts, vec!["a", "b", "c"]);
    }
}

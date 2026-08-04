use std::ffi::{c_char, CStr, CString};

/// Translate a complete-object constructor mangled name (`...C2...`) to the
/// base-object constructor spelling (`...C1...`).
///
/// Faithful port of `HookManager::translateConstructorName` (`hook.cpp:277-303`):
/// after `_Z`, skips `N` markers and `length+identifier` groups; if the
/// remainder starts with `C2`, rewrites that `2` to `1`. Returns `None` when
/// there is nothing to translate (non-`_Z` name, no `C2` suffix reached).
pub fn translate_constructor_name(name: &str) -> Option<String> {
    let bytes = name.as_bytes();
    if bytes.len() < 2 || &bytes[..2] != b"_Z" {
        return None;
    }
    let mut s = 2usize;
    while s < bytes.len() {
        if bytes[s] == b'N' {
            s += 1;
            continue;
        }
        let start = s;
        while s < bytes.len() && bytes[s].is_ascii_digit() {
            s += 1;
        }
        if s == start {
            break;
        }
        let mut len = 0i64;
        for &d in &bytes[start..s] {
            len = len * 10 + (d - b'0') as i64;
        }
        s += len as usize;
    }
    if s >= bytes.len() {
        return None;
    }
    if bytes.len() - s >= 2 && bytes[s] == b'C' && bytes[s + 1] == b'2' {
        let mut out = bytes.to_vec();
        out[s + 1] = b'1';
        return Some(String::from_utf8(out).ok()?);
    }
    None
}

/// `#[no_mangle]` twin of `HookManager::translateConstructorName`
/// (`hook.cpp:277`). Returns a freshly-allocated (never freed) C string, or
/// NULL when there is nothing to translate.
#[no_mangle]
pub unsafe extern "C" fn translateConstructorName(name: *const c_char) -> *mut c_char {
    if name.is_null() {
        return std::ptr::null_mut();
    }
    let name = match CStr::from_ptr(name).to_str() {
        Ok(s) => s,
        Err(_) => return std::ptr::null_mut(),
    };
    match translate_constructor_name(name) {
        Some(translated) => {
            let c = CString::new(translated).expect("no interior NUL");
            c.into_raw()
        }
        None => std::ptr::null_mut(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translate_simple_ctor() {
        assert_eq!(translate_constructor_name("_ZN3FooC2Ev").as_deref(), Some("_ZN3FooC1Ev"));
    }

    #[test]
    fn translate_nested_ctor() {
        assert_eq!(
            translate_constructor_name("_ZN3Foo4implC2Ev").as_deref(),
            Some("_ZN3Foo4implC1Ev")
        );
    }

    #[test]
    fn translate_length_greater_than_9() {
        // 21-char identifier ("AppPlatform_android23"), length prefix 21.
        assert_eq!(
            translate_constructor_name("_ZN21AppPlatform_android23C2Ev").as_deref(),
            Some("_ZN21AppPlatform_android23C1Ev")
        );
    }

    #[test]
    fn no_translate_already_c1() {
        assert_eq!(translate_constructor_name("_ZN3FooC1Ev"), None);
    }

    #[test]
    fn no_translate_non_mangled() {
        assert_eq!(translate_constructor_name("foo"), None);
        assert_eq!(translate_constructor_name("_Z"), None);
    }

    #[test]
    fn no_translate_no_ctor_suffix() {
        assert_eq!(translate_constructor_name("_ZN3FooEv"), None);
    }
}
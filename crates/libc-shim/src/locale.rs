#![allow(non_camel_case_types, unused)]

use std::ffi::{c_char, CStr, CString};
use std::os::raw::c_void;
use std::ptr;

/// Android/MCPE often builds locale names as `lang.encoding` (e.g. `en.UTF-8`)
/// without a country code. glibc does not accept those, and many systems also
/// lack `en_US.UTF-8`. Fall back to a locale that actually exists.
fn rewrite_android_locale(name: &str) -> Option<&'static str> {
    if name.is_empty() || name == "C" || name == "POSIX" {
        return None;
    }
    // Already a known-good form — still may fail on the host; caller retries.
    if name.eq_ignore_ascii_case("C.UTF-8")
        || name.eq_ignore_ascii_case("C.utf8")
        || name.eq_ignore_ascii_case("en_US.UTF-8")
        || name.eq_ignore_ascii_case("en_US.utf8")
    {
        return None;
    }
    // `en.UTF-8`, `de.utf8`, etc. → prefer C.UTF-8 (matches mcpelauncher workaround)
    if let Some((lang, enc)) = name.split_once('.') {
        if !lang.is_empty()
            && !lang.contains('_')
            && !lang.contains('-')
            && (enc.eq_ignore_ascii_case("UTF-8") || enc.eq_ignore_ascii_case("utf8"))
        {
            return Some("C.UTF-8");
        }
    }
    // Other weird names: still try C.UTF-8 rather than failing hard
    Some("C.UTF-8")
}

/// Host locales to try when the requested name is unavailable.
const LOCALE_FALLBACKS: &[&str] = &["C.UTF-8", "C.utf8", "C", "POSIX", "en_US.UTF-8", "en_US.utf8"];

unsafe fn try_newlocale(
    category_mask: i32,
    locale: *const c_char,
    base: *mut c_void,
) -> *mut c_void {
    extern "C" {
        fn newlocale(
            category_mask: i32,
            locale: *const c_char,
            base: *mut c_void,
        ) -> *mut c_void;
    }
    newlocale(category_mask, locale, base)
}

pub unsafe extern "C" fn setlocale(category: i32, locale: *const c_char) -> *mut c_char {
    if locale.is_null() {
        return libc::setlocale(category, locale);
    }
    let name = CStr::from_ptr(locale).to_string_lossy();
    let mut r = libc::setlocale(category, locale);
    if !r.is_null() {
        return r;
    }
    if let Some(rewritten) = rewrite_android_locale(&name) {
        if let Ok(c) = CString::new(rewritten) {
            r = libc::setlocale(category, c.as_ptr());
            if !r.is_null() {
                return r;
            }
        }
    }
    for fb in LOCALE_FALLBACKS {
        if let Ok(c) = CString::new(*fb) {
            r = libc::setlocale(category, c.as_ptr());
            if !r.is_null() {
                return r;
            }
        }
    }
    // Last resort: leave locale unchanged query
    libc::setlocale(category, ptr::null())
}

pub unsafe extern "C" fn localeconv() -> *mut libc::lconv {
    libc::localeconv()
}

pub unsafe extern "C" fn newlocale(
    category_mask: i32,
    locale: *const c_char,
    base: *mut c_void,
) -> *mut c_void {
    // Preserve base when provided (previously always discarded).
    let base = if base.is_null() { ptr::null_mut() } else { base };

    if locale.is_null() {
        return try_newlocale(category_mask, locale, base);
    }

    let name = CStr::from_ptr(locale).to_string_lossy();
    let mut r = try_newlocale(category_mask, locale, base);
    if !r.is_null() {
        return r;
    }

    if let Some(rewritten) = rewrite_android_locale(&name) {
        if let Ok(c) = CString::new(rewritten) {
            r = try_newlocale(category_mask, c.as_ptr(), base);
            if !r.is_null() {
                eprintln!(
                    "[locale-shim] newlocale({:?}) failed; using {:?}",
                    name, rewritten
                );
                return r;
            }
        }
    }

    for fb in LOCALE_FALLBACKS {
        if let Ok(c) = CString::new(*fb) {
            r = try_newlocale(category_mask, c.as_ptr(), base);
            if !r.is_null() {
                eprintln!(
                    "[locale-shim] newlocale({:?}) failed; using fallback {:?}",
                    name, fb
                );
                return r;
            }
        }
    }

    // Keep original failure semantics if nothing works
    try_newlocale(category_mask, locale, base)
}

pub unsafe extern "C" fn uselocale(locale: *mut c_void) -> *mut c_void {
    extern "C" {
        fn uselocale(locale: *mut c_void) -> *mut c_void;
    }
    uselocale(locale)
}

pub unsafe extern "C" fn freelocale(locale: *mut c_void) {
    extern "C" {
        fn freelocale(locale: *mut c_void);
    }
    freelocale(locale);
}

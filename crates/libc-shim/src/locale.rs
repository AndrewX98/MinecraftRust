#![allow(non_camel_case_types, unused)]

use std::ffi::c_char;

pub unsafe extern "C" fn setlocale(category: i32, locale: *const c_char) -> *mut c_char { libc::setlocale(category, locale) }
pub unsafe extern "C" fn localeconv() -> *mut libc::lconv { libc::localeconv() }
pub unsafe extern "C" fn newlocale(category_mask: i32, locale: *const c_char, _base: *mut std::ffi::c_void) -> *mut std::ffi::c_void {
    extern "C" { fn newlocale(category_mask: i32, locale: *const c_char, base: *mut std::ffi::c_void) -> *mut std::ffi::c_void; }
    newlocale(category_mask, locale, std::ptr::null_mut())
}
pub unsafe extern "C" fn uselocale(locale: *mut std::ffi::c_void) -> *mut std::ffi::c_void {
    extern "C" { fn uselocale(locale: *mut std::ffi::c_void) -> *mut std::ffi::c_void; }
    uselocale(locale)
}
pub unsafe extern "C" fn freelocale(locale: *mut std::ffi::c_void) {
    extern "C" { fn freelocale(locale: *mut std::ffi::c_void); }
    freelocale(locale);
}

#![allow(non_camel_case_types, unused)]

use std::ffi::c_void;

#[cfg(not(target_os = "macos"))]
extern "C" {
    #[link_name = "__errno_location"]
    fn glibc_errno_location() -> *mut i32;
}

#[cfg(target_os = "macos")]
extern "C" {
    #[link_name = "__error"]
    fn apple_error() -> *mut i32;
}

pub fn host_errno_location() -> *mut i32 {
    #[cfg(not(target_os = "macos"))]
    unsafe { glibc_errno_location() }
    #[cfg(target_os = "macos")]
    unsafe { apple_error() }
}

pub unsafe extern "C" fn __errno() -> *mut i32 {
    host_errno_location()
}

pub unsafe extern "C" fn __set_errno(val: i32) -> i32 {
    *host_errno_location() = val;
    val
}

pub(crate) fn sync_host_errno(r: i64) -> i64 {
    if r < 0 { unsafe { *__errno() = *host_errno_location(); } }
    r
}

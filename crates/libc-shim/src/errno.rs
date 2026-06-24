#![allow(non_camel_case_types, unused)]

use std::ffi::c_void;

extern "C" {
    #[link_name = "__errno_location"]
    fn glibc_errno_location() -> *mut i32;
}

pub unsafe extern "C" fn __errno() -> *mut i32 {
    glibc_errno_location()
}

pub unsafe extern "C" fn __set_errno(val: i32) -> i32 {
    *glibc_errno_location() = val;
    val
}

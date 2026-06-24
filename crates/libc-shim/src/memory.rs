#![allow(non_camel_case_types, unused)]

use std::ffi::{c_char, c_void};

pub unsafe extern "C" fn memcpy(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void { libc::memcpy(dst, src, n) }
pub unsafe extern "C" fn __memcpy_chk(dst: *mut c_void, src: *const c_void, size: usize, max_len: usize) -> *mut c_void {
    if size > max_len { eprintln!("memcpy overflow"); std::process::abort(); }
    libc::memcpy(dst, src, size)
}
pub unsafe extern "C" fn memmove(dst: *mut c_void, src: *const c_void, n: usize) -> *mut c_void { libc::memmove(dst, src, n) }
pub unsafe extern "C" fn __memmove_chk(dst: *mut c_void, src: *const c_void, size: usize, max_len: usize) -> *mut c_void {
    if size > max_len { eprintln!("memmove overflow"); std::process::abort(); }
    libc::memmove(dst, src, size)
}
pub unsafe extern "C" fn memset(s: *mut c_void, c: i32, n: usize) -> *mut c_void { libc::memset(s, c, n) }
pub unsafe extern "C" fn __memset_chk(dst: *mut c_void, c: i32, size: usize, max_len: usize) -> *mut c_void {
    if size > max_len { eprintln!("memset overflow"); std::process::abort(); }
    libc::memset(dst, c, size)
}
pub unsafe extern "C" fn memcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 { libc::memcmp(s1, s2, n) }
pub unsafe extern "C" fn memchr(s: *mut c_void, c: i32, n: usize) -> *mut c_void { libc::memchr(s, c, n) }
pub unsafe extern "C" fn memmem(haystack: *const c_void, hlen: usize, needle: *const c_void, nlen: usize) -> *mut c_void { libc::memmem(haystack, hlen, needle, nlen) }
pub unsafe extern "C" fn bcmp(s1: *const c_void, s2: *const c_void, n: usize) -> i32 {
    let r = libc::memcmp(s1, s2, n);
    if r < 0 { -1 } else if r > 0 { 1 } else { 0 }
}
pub unsafe extern "C" fn bcopy(src: *const c_void, dst: *mut c_void, n: usize) { libc::memmove(dst, src, n); }
pub unsafe extern "C" fn bzero(s: *mut c_void, n: usize) { libc::memset(s, 0, n); }
pub unsafe extern "C" fn ffs(i: i32) -> i32 {
    if i == 0 { return 0; }
    let mut v = i as u32;
    let mut r = 1;
    while (v & 1) == 0 { v >>= 1; r += 1; }
    r
}
pub unsafe extern "C" fn index(s: *mut c_char, c: i32) -> *mut c_char { libc::strchr(s, c) }
pub unsafe extern "C" fn rindex(s: *mut c_char, c: i32) -> *mut c_char { libc::strrchr(s, c) }

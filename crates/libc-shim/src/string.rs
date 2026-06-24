#![allow(non_camel_case_types, unused)]

use std::ffi::{c_char, c_void};

extern "C" {
    #[link_name = "strncat"]
    fn libc_strncat(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char;
}

pub unsafe extern "C" fn strlen(s: *const c_char) -> usize { libc::strlen(s) }
pub unsafe extern "C" fn __strlen_chk(s: *const c_char, max_len: usize) -> usize {
    let l = libc::strlen(s);
    if l >= max_len { eprintln!("__strlen_chk: string longer than expected"); std::process::abort(); }
    l
}
pub unsafe extern "C" fn strcmp(s1: *const c_char, s2: *const c_char) -> i32 { libc::strcmp(s1, s2) }
pub unsafe extern "C" fn strncmp(s1: *const c_char, s2: *const c_char, n: usize) -> i32 { libc::strncmp(s1, s2, n) }
pub unsafe extern "C" fn strcpy(dst: *mut c_char, src: *const c_char) -> *mut c_char { libc::strcpy(dst, src) }
pub unsafe extern "C" fn __strcpy_chk(dst: *mut c_char, src: *const c_char, _dst_len: usize) -> *mut c_char { libc::strcpy(dst, src) }
pub unsafe extern "C" fn strncpy(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char { libc::strncpy(dst, src, n) }
pub unsafe extern "C" fn __strncpy_chk(dst: *mut c_char, src: *const c_char, len: usize, _dst_len: usize) -> *mut c_char { libc::strncpy(dst, src, len) }
pub unsafe extern "C" fn strcat(dst: *mut c_char, src: *const c_char) -> *mut c_char { libc::strcat(dst, src) }
pub unsafe extern "C" fn __strcat_chk(dst: *mut c_char, src: *const c_char, _dst_len: usize) -> *mut c_char { libc::strcat(dst, src) }
pub unsafe extern "C" fn strncat(dst: *mut c_char, src: *const c_char, n: usize) -> *mut c_char { libc_strncat(dst, src, n) }
pub unsafe extern "C" fn __strncat_chk(dst: *mut c_char, src: *const c_char, len: usize, _dst_len: usize) -> *mut c_char { libc_strncat(dst, src, len) }
pub unsafe extern "C" fn strdup(s: *const c_char) -> *mut c_char { libc::strdup(s) }
pub unsafe extern "C" fn strndup(s: *const c_char, n: usize) -> *mut c_char { libc::strndup(s, n) }
pub unsafe extern "C" fn strchr(s: *mut c_char, c: i32) -> *mut c_char { libc::strchr(s, c) }
pub unsafe extern "C" fn strrchr(s: *mut c_char, c: i32) -> *mut c_char { libc::strrchr(s, c) }
pub unsafe extern "C" fn strstr(haystack: *mut c_char, needle: *const c_char) -> *mut c_char { libc::strstr(haystack, needle) }
pub unsafe extern "C" fn strtok(s: *mut c_char, delim: *const c_char) -> *mut c_char { libc::strtok(s, delim) }
pub unsafe extern "C" fn strtok_r(s: *mut c_char, delim: *const c_char, saveptr: *mut *mut c_char) -> *mut c_char { libc::strtok_r(s, delim, saveptr) }
pub unsafe extern "C" fn strerror(errnum: i32) -> *mut c_char { libc::strerror(errnum) }
pub unsafe extern "C" fn strerror_r(errnum: i32, buf: *mut c_char, buflen: usize) -> i32 { libc::strerror_r(errnum, buf, buflen) }
pub unsafe extern "C" fn strnlen(s: *const c_char, maxlen: usize) -> usize { libc::strnlen(s, maxlen) }
pub unsafe extern "C" fn strlcpy(dst: *mut c_char, src: *const c_char, size: usize) -> usize {
    let src_len = libc::strlen(src);
    if size > 0 {
        let copy_len = if src_len >= size { size - 1 } else { src_len };
        libc::memcpy(dst as *mut c_void, src as *const c_void, copy_len);
        *dst.add(copy_len) = 0;
    }
    src_len
}
pub unsafe extern "C" fn strcasecmp(s1: *const c_char, s2: *const c_char) -> i32 { libc::strcasecmp(s1, s2) }
pub unsafe extern "C" fn strncasecmp(s1: *const c_char, s2: *const c_char, n: usize) -> i32 { libc::strncasecmp(s1, s2, n) }
pub unsafe extern "C" fn strcoll(s1: *const c_char, s2: *const c_char) -> i32 { libc::strcoll(s1, s2) }
pub unsafe extern "C" fn strxfrm(dst: *mut c_char, src: *const c_char, n: usize) -> usize { libc::strxfrm(dst, src, n) }
pub unsafe extern "C" fn strsep(stringp: *mut *mut c_char, delim: *const c_char) -> *mut c_char {
    if stringp.is_null() || (*stringp).is_null() { return std::ptr::null_mut(); }
    let s = *stringp;
    let p = libc::strpbrk(s, delim);
    let ret = if p.is_null() {
        *stringp = std::ptr::null_mut();
        s
    } else {
        *p = 0;
        *stringp = p.add(1);
        s
    };
    ret
}
pub unsafe extern "C" fn strcspn(s: *const c_char, reject: *const c_char) -> usize { libc::strcspn(s, reject) }
pub unsafe extern "C" fn strpbrk(s: *mut c_char, accept: *const c_char) -> *mut c_char { libc::strpbrk(s, accept) }
pub unsafe extern "C" fn strspn(s: *const c_char, accept: *const c_char) -> usize { libc::strspn(s, accept) }
pub unsafe extern "C" fn strsignal(sig: i32) -> *mut c_char { libc::strsignal(sig) }

pub unsafe extern "C" fn memccpy(dst: *mut c_void, src: *const c_void, c: i32, n: usize) -> *mut c_void { libc::memccpy(dst, src, c, n) }
pub unsafe extern "C" fn basename(path: *const c_char) -> *mut c_char {
    extern "C" { fn __xpg_basename(path: *const c_char) -> *mut c_char; }
    __xpg_basename(path)
}

pub unsafe extern "C" fn __strchr_chk(s: *mut c_char, c: i32, _len: usize) -> *mut c_char { libc::strchr(s, c) }
pub unsafe extern "C" fn __strrchr_chk(s: *mut c_char, c: i32, _len: usize) -> *mut c_char { libc::strrchr(s, c) }
pub unsafe extern "C" fn __strncpy_chk2(dst: *mut c_char, src: *const c_char, len: usize, _src_len: usize, _dst_len: usize) -> *mut c_char { libc::strncpy(dst, src, len) }
pub unsafe extern "C" fn __strlcpy_chk(dst: *mut c_char, src: *const c_char, size: usize, _dst_len: usize) -> usize { strlcpy(dst, src, size) }

// locale-aware string variants (ignore locale, use C locale)
pub unsafe extern "C" fn strcoll_l(s1: *const c_char, s2: *const c_char, _l: *mut c_void) -> i32 { libc::strcoll(s1, s2) }
pub unsafe extern "C" fn strxfrm_l(dst: *mut c_char, src: *const c_char, n: usize, _l: *mut c_void) -> usize { libc::strxfrm(dst, src, n) }

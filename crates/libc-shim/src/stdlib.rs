#![allow(non_camel_case_types, unused)]

use std::ffi::{c_char, c_void};
use crate::errno::__errno;

pub unsafe extern "C" fn atexit(func: Option<extern "C" fn()>) -> i32 {
    match func {
        Some(f) => libc::atexit(f),
        None => -1,
    }
}

pub unsafe extern "C" fn exit(status: i32) {
    libc::exit(status);
}

pub unsafe extern "C" fn _Exit(status: i32) {
    libc::_exit(status);
}

pub unsafe extern "C" fn system(cmd: *const c_char) -> i32 {
    libc::system(cmd)
}

pub unsafe extern "C" fn getenv(name: *const c_char) -> *mut c_char {
    libc::getenv(name)
}

pub unsafe extern "C" fn putenv(string: *mut c_char) -> i32 {
    let r = libc::putenv(string);
    if r != 0 { *__errno() = *libc::__errno_location(); }
    r
}

pub unsafe extern "C" fn setenv(name: *const c_char, value: *const c_char, overwrite: i32) -> i32 {
    let r = libc::setenv(name, value, overwrite);
    if r != 0 { *__errno() = *libc::__errno_location(); }
    r
}

pub unsafe extern "C" fn unsetenv(name: *const c_char) -> i32 {
    let r = libc::unsetenv(name);
    if r != 0 { *__errno() = *libc::__errno_location(); }
    r
}

pub unsafe extern "C" fn realpath(path: *const c_char, resolved: *mut c_char) -> *mut c_char {
    libc::realpath(path, resolved)
}

pub unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
    libc::malloc(size)
}

pub unsafe extern "C" fn free(ptr: *mut c_void) {
    libc::free(ptr)
}

pub unsafe extern "C" fn calloc(nmemb: usize, size: usize) -> *mut c_void {
    libc::calloc(nmemb, size)
}

pub unsafe extern "C" fn realloc(ptr: *mut c_void, size: usize) -> *mut c_void {
    libc::realloc(ptr, size)
}

pub unsafe extern "C" fn memalign(alignment: usize, size: usize) -> *mut c_void {
    let mut ptr = std::ptr::null_mut();
    let r = libc::posix_memalign(&mut ptr, alignment, size);
    if r != 0 { std::ptr::null_mut() } else { ptr }
}

pub unsafe extern "C" fn posix_memalign(memptr: *mut *mut c_void, alignment: usize, size: usize) -> i32 {
    libc::posix_memalign(memptr, alignment, size)
}

pub unsafe extern "C" fn malloc_usable_size(ptr: *mut c_void) -> usize {
    libc::malloc_usable_size(ptr)
}

pub unsafe extern "C" fn valloc(size: usize) -> *mut c_void {
    let page_size = 4096;
    let aligned_size = (size + page_size - 1) & !(page_size - 1);
    let mut ptr = std::ptr::null_mut();
    let r = libc::posix_memalign(&mut ptr, page_size, aligned_size);
    if r != 0 { std::ptr::null_mut() } else { ptr }
}

// C++ operator new/delete
pub unsafe extern "C" fn _Znwj(size: u32) -> *mut c_void {
    libc::malloc(size as usize)
}
pub unsafe extern "C" fn _Znaj(size: u32) -> *mut c_void {
    libc::malloc(size as usize)
}
pub unsafe extern "C" fn _Znwm(size: usize) -> *mut c_void {
    libc::malloc(size)
}
pub unsafe extern "C" fn _Znam(size: usize) -> *mut c_void {
    libc::malloc(size)
}
pub unsafe extern "C" fn _ZdlPv(ptr: *mut c_void) {
    libc::free(ptr)
}
pub unsafe extern "C" fn _ZdaPv(ptr: *mut c_void) {
    libc::free(ptr)
}
pub unsafe extern "C" fn _ZnwjSt11align_val_t(size: u32, _align: usize) -> *mut c_void {
    libc::malloc(size as usize)
}
pub unsafe extern "C" fn _ZnwmSt11align_val_t(size: usize, _align: usize) -> *mut c_void {
    libc::malloc(size)
}

// string->number
pub unsafe extern "C" fn atoi(s: *const c_char) -> i32 { libc::atoi(s) }
pub unsafe extern "C" fn atol(s: *const c_char) -> i64 { libc::atol(s) }
pub unsafe extern "C" fn atoll(s: *const c_char) -> i64 { libc::atoll(s) }
pub unsafe extern "C" fn atof(s: *const c_char) -> f64 { libc::atof(s) }
pub unsafe extern "C" fn strtol(s: *const c_char, endptr: *mut *mut c_char, base: i32) -> i64 { libc::strtol(s, endptr, base) }
pub unsafe extern "C" fn strtoul(s: *const c_char, endptr: *mut *mut c_char, base: i32) -> u64 { libc::strtoul(s, endptr, base) }
pub unsafe extern "C" fn strtoll(s: *const c_char, endptr: *mut *mut c_char, base: i32) -> i64 { libc::strtoll(s, endptr, base) }
pub unsafe extern "C" fn strtoull(s: *const c_char, endptr: *mut *mut c_char, base: i32) -> u64 { libc::strtoull(s, endptr, base) }
pub unsafe extern "C" fn strtof(s: *const c_char, endptr: *mut *mut c_char) -> f32 { libc::strtof(s, endptr) }
pub unsafe extern "C" fn strtod(s: *const c_char, endptr: *mut *mut c_char) -> f64 { libc::strtod(s, endptr) }
pub unsafe extern "C" fn strtold(s: *const c_char, endptr: *mut *mut c_char) -> u128 {
    extern "C" { fn strtold(s: *const c_char, endptr: *mut *mut c_char) -> u128; }
    strtold(s, endptr)
}
pub unsafe extern "C" fn strtoq(s: *const c_char, endptr: *mut *mut c_char, base: i32) -> i64 { libc::strtoll(s, endptr, base) }
pub unsafe extern "C" fn strtouq(s: *const c_char, endptr: *mut *mut c_char, base: i32) -> u64 { libc::strtoull(s, endptr, base) }
pub unsafe extern "C" fn strtoimax(s: *const c_char, endptr: *mut *mut c_char, base: i32) -> i64 { libc::strtoll(s, endptr, base) }
pub unsafe extern "C" fn strtoumax(s: *const c_char, endptr: *mut *mut c_char, base: i32) -> u64 { libc::strtoull(s, endptr, base) }

pub unsafe extern "C" fn _exit(status: i32) { libc::_exit(status) }
pub unsafe extern "C" fn abs(j: i32) -> i32 { libc::abs(j) }
pub unsafe extern "C" fn ldexp(x: f64, exp: i32) -> f64 {
    extern "C" { fn ldexp(x: f64, exp: i32) -> f64; }
    ldexp(x, exp)
}
pub unsafe extern "C" fn bsearch(key: *const c_void, base: *const c_void, nmemb: usize, size: usize, compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> i32>) -> *mut c_void {
    if let Some(f) = compar { libc::bsearch(key, base, nmemb, size, Some(f)) } else { std::ptr::null_mut() }
}
pub unsafe extern "C" fn qsort(base: *mut c_void, nmemb: usize, size: usize, compar: Option<unsafe extern "C" fn(*const c_void, *const c_void) -> i32>) {
    if let Some(f) = compar { libc::qsort(base, nmemb, size, Some(f)); }
}

// locale-aware strto* variants (ignore locale, use C locale)
pub unsafe extern "C" fn strtoul_l(s: *const c_char, endptr: *mut *mut c_char, base: i32, _l: *mut c_void) -> u64 { libc::strtoul(s, endptr, base) }
pub unsafe extern "C" fn strtoll_l(s: *const c_char, endptr: *mut *mut c_char, base: i32, _l: *mut c_void) -> i64 { libc::strtoll(s, endptr, base) }
pub unsafe extern "C" fn strtoull_l(s: *const c_char, endptr: *mut *mut c_char, base: i32, _l: *mut c_void) -> u64 { libc::strtoull(s, endptr, base) }
pub unsafe extern "C" fn strtof_l(s: *const c_char, endptr: *mut *mut c_char, _l: *mut c_void) -> f32 { libc::strtof(s, endptr) }
pub unsafe extern "C" fn strtold_l(s: *const c_char, endptr: *mut *mut c_char, _l: *mut c_void) -> u128 {
    extern "C" { fn strtold(s: *const c_char, endptr: *mut *mut c_char) -> u128; }
    strtold(s, endptr)
}

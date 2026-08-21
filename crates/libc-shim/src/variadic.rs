//! Rust port of `variadic.c` (C-variadic shim wrappers).
//!
//! The C file existed because only C could do `va_start`/`va_end` on a
//! variadic call. With the crate on nightly `c_variadic`, the shims become
//! `unsafe extern "C" fn (..., mut args: ...)` — the desugared `VaList` is
//! ABI-compatible with the platform `va_list` (on x86_64 it is
//! `#[repr(transparent)]` over the `__va_list_tag` struct, passed indirectly
//! in non-Rust ABIs), so a pointer to it is handed to the `v*` implementations
//! in `crate::stdio`. Each shim stays a private symbol (referenced by pointer
//! from `lib.rs`'s symbol table), matching the old file-local `static`
//! C functions so no global symbol collides.

use std::ffi::{c_char, c_int, c_long, c_void, VaList};

use crate::file::BionicFile;

/// glibc `va_list` arrives as a pointer to the `__va_list_tag` array — matches
/// the `type va_list = *mut c_void` convention used by `crate::stdio`.
type VaListPtr = *mut c_void;

fn va_ptr(args: &mut VaList) -> VaListPtr {
    args as *mut VaList as *mut c_void
}

// v* entry points not already exposed by crate::stdio
extern "C" {
    fn vfprintf(stream: *mut libc::FILE, fmt: *const c_char, ap: VaListPtr) -> c_int;
    fn vfscanf(stream: *mut libc::FILE, fmt: *const c_char, ap: VaListPtr) -> c_int;
    fn vswprintf(wcs: *mut i32, maxlen: usize, fmt: *const i32, ap: VaListPtr) -> c_int;
}

pub unsafe extern "C" fn shim_sscanf(s: *const c_char, fmt: *const c_char, mut args: ...) -> c_int {
    let ap = va_ptr(&mut args);
    crate::stdio::vsscanf(s, fmt, ap)
}

pub unsafe extern "C" fn shim_printf(fmt: *const c_char, mut args: ...) -> c_int {
    let ap = va_ptr(&mut args);
    crate::stdio::vprintf(fmt, ap)
}

pub unsafe extern "C" fn shim_sprintf(buf: *mut c_char, fmt: *const c_char, mut args: ...) -> c_int {
    let ap = va_ptr(&mut args);
    crate::stdio::vsprintf(buf, fmt, ap)
}

pub unsafe extern "C" fn shim_snprintf(buf: *mut c_char, size: usize, fmt: *const c_char, mut args: ...) -> c_int {
    let ap = va_ptr(&mut args);
    crate::stdio::vsnprintf(buf, size, fmt, ap)
}

pub unsafe extern "C" fn shim_asprintf(s: *mut *mut c_char, fmt: *const c_char, mut args: ...) -> c_int {
    let ap = va_ptr(&mut args);
    crate::stdio::vasprintf(s, fmt, ap)
}

pub unsafe extern "C" fn shim___snprintf_chk(
    buf: *mut c_char,
    size: usize,
    _flags: c_int,
    _dst_len: usize,
    fmt: *const c_char,
    mut args: ...,
) -> c_int {
    let ap = va_ptr(&mut args);
    crate::stdio::vsnprintf(buf, size, fmt, ap)
}

pub unsafe extern "C" fn shim_scanf(fmt: *const c_char, mut args: ...) -> c_int {
    let ap = va_ptr(&mut args);
    crate::stdio::vscanf(fmt, ap)
}

pub unsafe extern "C" fn shim_swprintf(wcs: *mut i32, maxlen: usize, fmt: *const i32, mut args: ...) -> c_int {
    let ap = va_ptr(&mut args);
    vswprintf(wcs, maxlen, fmt, ap)
}

pub unsafe extern "C" fn shim_syscall(number: c_long, mut args: ...) -> c_long {
    let a1 = unsafe { args.next_arg::<c_long>() };
    let a2 = unsafe { args.next_arg::<c_long>() };
    let a3 = unsafe { args.next_arg::<c_long>() };
    let a4 = unsafe { args.next_arg::<c_long>() };
    let a5 = unsafe { args.next_arg::<c_long>() };
    let a6 = unsafe { args.next_arg::<c_long>() };
    libc::syscall(number as _, a1, a2, a3, a4, a5, a6) as c_long
}

pub unsafe extern "C" fn shim_fprintf(fp: *mut BionicFile, fmt: *const c_char, mut args: ...) -> c_int {
    let ap = va_ptr(&mut args);
    vfprintf((*fp).wrapped, fmt, ap)
}

pub unsafe extern "C" fn shim_fscanf(fp: *mut BionicFile, fmt: *const c_char, mut args: ...) -> c_int {
    let ap = va_ptr(&mut args);
    let r = vfscanf((*fp).wrapped, fmt, ap);
    (*fp)._flags = if libc::feof((*fp).wrapped) != 0 { 0x0020 } else { 0 };
    r
}

#![allow(non_camel_case_types, unused)]

use std::ffi::c_char;

pub unsafe extern "C" fn isalnum(c: i32) -> i32 { libc::isalnum(c) }
pub unsafe extern "C" fn isalpha(c: i32) -> i32 { libc::isalpha(c) }
pub unsafe extern "C" fn isblank(c: i32) -> i32 { libc::isblank(c) }
pub unsafe extern "C" fn iscntrl(c: i32) -> i32 { libc::iscntrl(c) }
pub unsafe extern "C" fn isdigit(c: i32) -> i32 { libc::isdigit(c) }
pub unsafe extern "C" fn isgraph(c: i32) -> i32 { libc::isgraph(c) }
pub unsafe extern "C" fn islower(c: i32) -> i32 { libc::islower(c) }
pub unsafe extern "C" fn isprint(c: i32) -> i32 { libc::isprint(c) }
pub unsafe extern "C" fn ispunct(c: i32) -> i32 { libc::ispunct(c) }
pub unsafe extern "C" fn isspace(c: i32) -> i32 { libc::isspace(c) }
pub unsafe extern "C" fn isupper(c: i32) -> i32 { libc::isupper(c) }
pub unsafe extern "C" fn isxdigit(c: i32) -> i32 { libc::isxdigit(c) }
pub unsafe extern "C" fn isascii(c: i32) -> i32 { (c >= 0 && c <= 127) as i32 }
pub unsafe extern "C" fn tolower(c: i32) -> i32 { libc::tolower(c) }
pub unsafe extern "C" fn toupper(c: i32) -> i32 { libc::toupper(c) }
// locale-aware ctype variants (ignore locale, use C locale)
pub unsafe extern "C" fn islower_l(c: i32, _l: *mut std::ffi::c_void) -> i32 { libc::islower(c) }
pub unsafe extern "C" fn isupper_l(c: i32, _l: *mut std::ffi::c_void) -> i32 { libc::isupper(c) }
pub unsafe extern "C" fn isxdigit_l(c: i32, _l: *mut std::ffi::c_void) -> i32 { libc::isxdigit(c) }
pub unsafe extern "C" fn isdigit_l(c: i32, _l: *mut std::ffi::c_void) -> i32 { libc::isdigit(c) }
pub unsafe extern "C" fn tolower_l(c: i32, _l: *mut std::ffi::c_void) -> i32 { libc::tolower(c) }
pub unsafe extern "C" fn toupper_l(c: i32, _l: *mut std::ffi::c_void) -> i32 { libc::toupper(c) }

pub unsafe extern "C" fn __ctype_get_mb_cur_max() -> usize {
    extern "C" { fn __ctype_get_mb_cur_max() -> usize; }
    __ctype_get_mb_cur_max()
}

// Bionic ctype bitmask flags
const _U: u8 = 0x01; // upper
const _L: u8 = 0x02; // lower
const _D: u8 = 0x04; // digit
const _S: u8 = 0x08; // space
const _P: u8 = 0x10; // punct
const _C: u8 = 0x20; // cntrl
const _X: u8 = 0x40; // hex
const _B: u8 = 0x80; // blank

// Helper: Sync wrapper for raw pointer (needed for static storage)
pub struct SyncConstPtr<T>(pub *const T);
unsafe impl<T> Sync for SyncConstPtr<T> {}

// Private data tables — actual ctype data arrays.
// Not exported directly; accessed via pointer variables below.
static _CTYPE_DATA: [u8; 257] = {
    let mut t = [0u8; 257];
    let mut i = 0u8;
    // index 0 = EOF
    t[0] = 0;
    loop {
        let c = i;
        let flags = match c {
            0x00..=0x08 => _C,
            0x09..=0x0D => _C | _S,
            0x0E..=0x1F => _C,
            0x20 => _S | _B,
            0x21..=0x2F => _P,
            0x30..=0x39 => _D | _X,
            0x3A..=0x40 => _P,
            0x41..=0x46 => _U | _X,
            0x47..=0x5A => _U,
            0x5B..=0x60 => _P,
            0x61..=0x66 => _L | _X,
            0x67..=0x7A => _L,
            0x7B..=0x7E => _P,
            0x7F => _C,
            _ => 0,
        };
        t[(c as usize) + 1] = flags;
        if i == 255 { break; }
        i += 1;
    }
    t
};

static _TOLOWER_DATA: [i16; 257] = {
    let mut t = [-1i16; 257];
    let mut i = 0u8;
    loop {
        t[(i as usize) + 1] = if i >= b'A' && i <= b'Z' {
            (i + 32) as i16
        } else {
            i as i16
        };
        if i == 255 { break; }
        i += 1;
    }
    t
};

static _TOUPPER_DATA: [i16; 257] = {
    let mut t = [-1i16; 257];
    let mut i = 0u8;
    loop {
        t[(i as usize) + 1] = if i >= b'a' && i <= b'z' {
            (i - 32) as i16
        } else {
            i as i16
        };
        if i == 255 { break; }
        i += 1;
    }
    t
};

// Pointer variables matching bionic's extern const char *_ctype_ pattern.
// The game declares _ctype_ as const char* (a pointer), not an array.
// The GOT entry stores the address of this pointer variable;
// the game reads the pointer value then indexes into the data.
// SyncConstPtr wraps the raw pointer so it can be used in static context.
pub static _CTYPE_PTR: SyncConstPtr<u8> = SyncConstPtr(&_CTYPE_DATA as *const u8);
pub static _TOLOWER_PTR: SyncConstPtr<i16> = SyncConstPtr(&_TOLOWER_DATA as *const i16);
pub static _TOUPPER_PTR: SyncConstPtr<i16> = SyncConstPtr(&_TOUPPER_DATA as *const i16);

// Keep #[no_mangle] arrays for direct ELF visibility (C++ code in the bridge).
#[no_mangle]
pub static _ctype_: [u8; 257] = _CTYPE_DATA;
#[no_mangle]
pub static _tolower_tab_: [i16; 257] = _TOLOWER_DATA;
#[no_mangle]
pub static _toupper_tab_: [i16; 257] = _TOUPPER_DATA;

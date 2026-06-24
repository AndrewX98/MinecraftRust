//! Data symbols: static data exported for bionic compatibility.
#![allow(non_camel_case_types, dead_code)]

use std::ffi::c_void;

// These can be zero-initialized stub values; the real game rarely inspects them.
#[no_mangle]
pub static mut __sF: [u8; 0x98 * 3] = [0u8; 0x98 * 3]; // 3 × bionic FILE (152 bytes on LP64)

// Provide stdin/stdout/stderr pointing into __sF.
pub static mut STDIN_PTR: *mut u8 = unsafe { __sF.as_ptr() as *mut u8 };
pub static mut STDOUT_PTR: *mut u8 = unsafe { __sF.as_ptr() as *mut u8 }.wrapping_add(0x98);
pub static mut STDERR_PTR: *mut u8 = unsafe { __sF.as_ptr() as *mut u8 }.wrapping_add(0x98 * 2);

#[no_mangle]
pub static mut __isthreaded: i32 = 1;

#[no_mangle]
pub static in6addr_any: [u8; 16] = [0u8; 16];

#[no_mangle]
pub static in6addr_loopback: [u8; 16] = [0u8; 16]; // ::1 — we can fill it

#[no_mangle]
pub static mut mallinfo_data: [i32; 10] = [0i32; 10]; // bionic mallinfo: 10 int32 fields

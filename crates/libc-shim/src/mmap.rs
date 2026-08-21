#![allow(non_camel_case_types, unused)]

use std::ffi::c_void;

pub unsafe extern "C" fn mmap(addr: *mut c_void, length: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut c_void {
    libc::mmap(addr, length, prot, flags, fd, offset)
}
pub unsafe extern "C" fn munmap(addr: *mut c_void, length: usize) -> i32 { libc::munmap(addr, length) }
pub unsafe extern "C" fn mprotect(addr: *mut c_void, length: usize, prot: i32) -> i32 { libc::mprotect(addr, length, prot) }
pub unsafe extern "C" fn madvise(addr: *mut c_void, length: usize, advice: i32) -> i32 { libc::madvise(addr, length, advice) }
pub unsafe extern "C" fn msync(addr: *mut c_void, length: usize, flags: i32) -> i32 { libc::msync(addr, length, flags) }
pub unsafe extern "C" fn mlock(addr: *const c_void, len: usize) -> i32 { libc::mlock(addr, len) }
pub unsafe extern "C" fn munlock(addr: *const c_void, len: usize) -> i32 { libc::munlock(addr, len) }
#[cfg(not(target_os = "macos"))]
pub unsafe extern "C" fn mremap(old_addr: *mut c_void, old_size: usize, new_size: usize, flags: i32) -> *mut c_void {
    extern "C" { fn mremap(old_addr: *mut c_void, old_size: usize, new_size: usize, flags: i32) -> *mut c_void; }
    mremap(old_addr, old_size, new_size, flags)
}
// Linux-only syscall; emulate as move+copy (MREMAP_MAYMOVE semantics)
#[cfg(target_os = "macos")]
pub unsafe extern "C" fn mremap(old_addr: *mut c_void, old_size: usize, new_size: usize, _flags: i32) -> *mut c_void {
    if old_addr.is_null() || new_size == 0 { crate::errno::__set_errno(libc::EINVAL); return libc::MAP_FAILED; }
    let prot = libc::PROT_READ | libc::PROT_WRITE;
    let new_addr = libc::mmap(std::ptr::null_mut(), new_size, prot, libc::MAP_PRIVATE | libc::MAP_ANON, -1, 0);
    if new_addr == libc::MAP_FAILED { return libc::MAP_FAILED; }
    libc::memcpy(new_addr, old_addr, old_size.min(new_size));
    libc::munmap(old_addr, old_size);
    new_addr
}

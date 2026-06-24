#![allow(non_camel_case_types, unused)]

use std::ffi::c_char;
use crate::types::HostStat;

pub unsafe extern "C" fn stat(path: *const c_char, buf: *mut HostStat) -> i32 {
    let mut host_buf: libc::stat = std::mem::zeroed();
    let r = libc::stat(path, &mut host_buf);
    if r == 0 { convert_stat(&host_buf, buf); }
    r
}

pub unsafe extern "C" fn fstat(fd: i32, buf: *mut HostStat) -> i32 {
    let mut host_buf: libc::stat = std::mem::zeroed();
    let r = libc::fstat(fd, &mut host_buf);
    if r == 0 { convert_stat(&host_buf, buf); }
    r
}

pub unsafe extern "C" fn lstat(path: *const c_char, buf: *mut HostStat) -> i32 {
    let mut host_buf: libc::stat = std::mem::zeroed();
    let r = libc::lstat(path, &mut host_buf);
    if r == 0 { convert_stat(&host_buf, buf); }
    r
}

pub unsafe extern "C" fn stat64(path: *const c_char, buf: *mut HostStat) -> i32 { stat(path, buf) }
pub unsafe extern "C" fn fstat64(fd: i32, buf: *mut HostStat) -> i32 { fstat(fd, buf) }
pub unsafe extern "C" fn lstat64(path: *const c_char, buf: *mut HostStat) -> i32 { lstat(path, buf) }

unsafe fn convert_stat(src: &libc::stat, dst: *mut HostStat) {
    (*dst).st_dev = src.st_dev as u64;
    (*dst).st_ino = src.st_ino as u64;
    (*dst).st_mode = src.st_mode;
    (*dst).st_nlink = src.st_nlink as u64;
    (*dst).st_uid = src.st_uid;
    (*dst).st_gid = src.st_gid;
    (*dst).st_rdev = src.st_rdev as u64;
    (*dst).st_size = src.st_size;
    (*dst).st_blksize = src.st_blksize;
    (*dst).st_blocks = src.st_blocks;
    (*dst).st_atim = libc::timespec { tv_sec: src.st_atime as _, tv_nsec: src.st_atime_nsec as _ };
    (*dst).st_mtim = libc::timespec { tv_sec: src.st_mtime as _, tv_nsec: src.st_mtime_nsec as _ };
    (*dst).st_ctim = libc::timespec { tv_sec: src.st_ctime as _, tv_nsec: src.st_ctime_nsec as _ };
}

pub unsafe extern "C" fn mkfifo(path: *const c_char, mode: u32) -> i32 { libc::mkfifo(path, mode) }
pub unsafe extern "C" fn mknod(path: *const c_char, mode: u32, dev: u64) -> i32 { libc::mknod(path, mode, dev) }
pub unsafe extern "C" fn utime(filename: *const c_char, times: *const libc::utimbuf) -> i32 { libc::utime(filename, times) }
pub unsafe extern "C" fn utimes(path: *const c_char, times: *const libc::timeval) -> i32 { libc::utimes(path, times) }
pub unsafe extern "C" fn utimensat(dirfd: i32, path: *const c_char, times: *const libc::timespec, flags: i32) -> i32 { libc::utimensat(dirfd, path, times, flags) }
pub unsafe extern "C" fn futimens(fd: i32, times: *const libc::timespec) -> i32 { libc::futimens(fd, times) }
pub unsafe extern "C" fn statfs(path: *const c_char, buf: *mut libc::statfs) -> i32 { libc::statfs(path, buf) }
pub unsafe extern "C" fn fstatfs(fd: i32, buf: *mut libc::statfs) -> i32 { libc::fstatfs(fd, buf) }
pub unsafe extern "C" fn statvfs(path: *const c_char, buf: *mut libc::statvfs) -> i32 { libc::statvfs(path, buf) }
pub unsafe extern "C" fn fstatvfs(fd: i32, buf: *mut libc::statvfs) -> i32 { libc::fstatvfs(fd, buf) }

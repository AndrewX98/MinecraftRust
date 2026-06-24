#![allow(non_camel_case_types, dead_code)]
use std::ffi::{c_char, c_int, c_void};

pub type off_t = i64;
pub type pid_t = i32;
pub type pthread_t = usize;

#[repr(C)]
pub struct bionic_stat {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub __pad0: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atim: libc::timespec,
    pub st_mtim: libc::timespec,
    pub st_ctim: libc::timespec,
    pub __pad3: [i64; 3],
}

#[repr(C)]
pub struct bionic_stat_aarch64 {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_mode: u32,
    pub st_nlink: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_rdev: u64,
    pub __pad0: u64,
    pub st_size: i64,
    pub st_blksize: i32,
    pub __pad1: i32,
    pub st_blocks: i64,
    pub st_atim: libc::timespec,
    pub st_mtim: libc::timespec,
    pub st_ctim: libc::timespec,
    pub __pad2: u32,
    pub __pad3: u32,
}

#[cfg(target_arch = "aarch64")]
pub type HostStat = bionic_stat_aarch64;
#[cfg(target_arch = "x86_64")]
pub type HostStat = bionic_stat;

#[repr(C)]
pub struct bionic_sockaddr_in {
    pub family: u16,
    pub port: u16,
    pub addr: u32,
    pub filler: [u8; 8],
}

#[repr(C)]
pub struct bionic_pthread_mutex_t {
    pub data: [i32; 10],
}

#[repr(C)]
pub struct bionic_pthread_cond_t {
    pub data: [i32; 12],
}

#[repr(C)]
pub struct bionic_pthread_rwlock_t {
    pub data: [i32; 14],
}

#[repr(C)]
pub struct bionic_pthread_attr_t {
    pub flags: u64,
    pub stack_base: *mut c_void,
    pub stack_size: usize,
    pub guard_size: usize,
    pub sched_policy: i32,
    pub sched_priority: i32,
    pub __padding: [i32; 4],
}

#[repr(C)]
pub struct bionic_pthread_mutexattr_t {
    pub type_: u32,
}

#[repr(C)]
pub struct bionic_pthread_condattr_t {
    pub shared: u32,
    pub clock: u32,
}

pub type bionic_pthread_key_t = u32;
pub type bionic_pthread_once_t = i32;

#[repr(C)]
pub struct bionic_sched_param {
    pub sched_priority: i32,
}

#[repr(C)]
pub struct bionic_addrinfo {
    pub ai_flags: i32,
    pub ai_family: i32,
    pub ai_socktype: i32,
    pub ai_protocol: i32,
    pub ai_addrlen: u32,
    pub ai_canonname: *mut c_char,
    pub ai_addr: *mut c_void,
    pub ai_next: *mut bionic_addrinfo,
}

#[repr(C)]
pub struct bionic_timeval {
    pub tv_sec: i64,
    pub tv_usec: i64,
}

#[repr(C)]
pub struct bionic_rlimit {
    pub rlim_cur: u64,
    pub rlim_max: u64,
}

#[repr(C)]
pub struct bionic_iovec {
    pub iov_base: *mut c_void,
    pub iov_len: usize,
}

#[repr(C)]
pub struct shimmed_symbol {
    pub name: *const c_char,
    pub value: *mut c_void,
}

extern "C" {
    pub fn getenv(name: *const c_char) -> *mut c_char;
    pub fn setenv(name: *const c_char, value: *const c_char, overwrite: i32) -> i32;
    pub fn unsetenv(name: *const c_char) -> i32;
}

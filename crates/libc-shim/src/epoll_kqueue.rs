//! macOS kqueue-backed implementation of the bionic epoll surface.
//!
//! Linux builds use direct libc passthroughs (`misc.rs`); this module is only
//! compiled on Darwin and translates the four `epoll_*` entry points onto
//! kqueue, mirroring what upstream's vendored `epoll-shim` provides.

#![allow(non_camel_case_types, unused)]

use std::collections::HashMap;
use std::sync::Mutex;

// bionic/glibc epoll constants (match <sys/epoll.h> values)
pub const EPOLLIN: u32 = 0x001;
pub const EPOLLPRI: u32 = 0x002;
pub const EPOLLOUT: u32 = 0x004;
pub const EPOLLERR: u32 = 0x008;
pub const EPOLLHUP: u32 = 0x010;
pub const EPOLLRDHUP: u32 = 0x2000;
pub const EPOLLET: u32 = 1 << 31;
pub const EPOLLONESHOT: u32 = 1 << 30;

const EPOLL_CTL_ADD: i32 = 1;
const EPOLL_CTL_DEL: i32 = 2;
const EPOLL_CTL_MOD: i32 = 3;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct epoll_event {
    pub events: u32,
    pub u64_: u64,
}

// host kevent layout (macOS): ident(uintptr), filter(int16), flags(u16),
// fflags(u32), data(intptr), udata(*mut c_void)
#[repr(C)]
#[derive(Clone, Copy)]
struct kevent {
    ident: usize,
    filter: i16,
    flags: u16,
    fflags: u32,
    data: isize,
    udata: *mut std::ffi::c_void,
}

extern "C" {
    fn kqueue() -> i32;
    fn kevent(
        kq: i32,
        changelist: *const kevent,
        nchanges: i32,
        eventlist: *mut kevent,
        nevents: i32,
        timeout: *const libc::timespec,
    ) -> i32;
}

const EV_ADD: u16 = 0x0001;
const EV_DELETE: u16 = 0x0002;
const EV_ENABLE: u16 = 0x0004;
const EV_DISABLE: u16 = 0x0008;
const EV_ONESHOT: u16 = 0x0010;
const EV_CLEAR: u16 = 0x0020;
const EV_EOF: u16 = 0x8000;
const EV_ERROR: u16 = 0x4000;

const EVFILT_READ: i16 = -1;
const EVFILT_WRITE: i16 = -2;
const EVFILT_VNODE: i16 = -4;
const EVFILT_EXCEPT: i16 = -15;

static REGISTRY: Mutex<Option<HashMap<i32, KqEntry>>> = Mutex::new(None);

struct KqEntry {
    kq_fd: i32,
}

fn with_registry<R>(f: impl FnOnce(&mut HashMap<i32, KqEntry>) -> R) -> R {
    let mut guard = REGISTRY.lock().unwrap();
    if guard.is_none() { *guard = Some(HashMap::new()); }
    f(guard.as_mut().unwrap())
}

fn events_to_filters(events: u32) -> Vec<(i16, u16)> {
    let mut out = Vec::new();
    let mut base_flags: u16 = EV_ADD | EV_CLEAR;
    if events & EPOLLET != 0 { base_flags |= EV_CLEAR; }
    if events & EPOLLONESHOT != 0 { base_flags |= EV_ONESHOT; }
    if events & (EPOLLIN | EPOLLPRI | EPOLLRDHUP) != 0 { out.push((EVFILT_READ, base_flags)); }
    if events & EPOLLOUT != 0 { out.push((EVFILT_WRITE, base_flags)); }
    if out.is_empty() { out.push((EVFILT_READ, base_flags | EV_DISABLE)); }
    out
}

fn filters_to_events(filter: i16, fflags: u32, flags: u16) -> u32 {
    match filter {
        EVFILT_READ => {
            let mut e = EPOLLIN;
            if flags & EV_EOF != 0 { e |= EPOLLHUP | EPOLLRDHUP; }
            e
        }
        EVFILT_WRITE => {
            let mut e = EPOLLOUT;
            if flags & EV_EOF != 0 { e |= EPOLLHUP; }
            e
        }
        EVFILT_EXCEPT => EPOLLPRI,
        _ => {
            let _ = fflags;
            EPOLLERR
        }
    }
}

fn make_kevent(ident: usize, filter: i16, flags: u16) -> kevent {
    kevent { ident, filter, flags, fflags: 0, data: 0, udata: std::ptr::null_mut() }
}

pub unsafe fn epoll_create_impl(_size: i32) -> i32 {
    epoll_create1_impl(0)
}

pub unsafe fn epoll_create1_impl(flags: i32) -> i32 {
    let fd = kqueue();
    if fd < 0 { return -1; }
    // mark close-on-exec like epoll_create1(EPOLL_CLOEXEC) callers expect
    if flags & 0o2000000 != 0 {
        libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC);
    }
    with_registry(|reg| { reg.insert(fd, KqEntry { kq_fd: fd }); });
    fd
}

pub unsafe fn epoll_ctl_impl(epfd: i32, op: i32, fd: i32, event: *mut epoll_event) -> i32 {
    let known = with_registry(|reg| reg.contains_key(&epfd));
    if !known { crate::errno::__set_errno(libc::EINVAL); return -1; }
    let changes: Vec<kevent> = match op {
        EPOLL_CTL_DEL => vec![make_kevent(fd as usize, EVFILT_READ, EV_DELETE), make_kevent(fd as usize, EVFILT_WRITE, EV_DELETE)],
        EPOLL_CTL_ADD | EPOLL_CTL_MOD => {
            let ev = if event.is_null() { epoll_event { events: 0, u64_: 0 } } else { *event };
            events_to_filters(ev.events).into_iter().map(|(f, fl)| make_kevent(fd as usize, f, fl)).collect()
        }
        _ => { crate::errno::__set_errno(libc::EINVAL); return -1; }
    };
    let ret = kevent(epfd, changes.as_ptr(), changes.len() as i32, std::ptr::null_mut(), 0, std::ptr::null());
    if ret < 0 { -1 } else { 0 }
}

pub unsafe fn epoll_wait_impl(epfd: i32, events: *mut epoll_event, maxevents: i32, timeout: i32) -> i32 {
    let known = with_registry(|reg| reg.contains_key(&epfd));
    if !known || maxevents <= 0 { crate::errno::__set_errno(libc::EINVAL); return -1; }
    let ts = if timeout < 0 {
        std::ptr::null()
    } else {
        &libc::timespec { tv_sec: (timeout / 1000) as _, tv_nsec: ((timeout % 1000) * 1_000_000) as _ }
    };
    let mut out: Vec<kevent> = vec![std::mem::zeroed(); maxevents as usize];
    let n = kevent(epfd, std::ptr::null(), 0, out.as_mut_ptr(), maxevents, ts);
    if n < 0 { return -1; }
    for i in 0..n as usize {
        let k = out[i];
        *events.add(i) = epoll_event {
            events: filters_to_events(k.filter, k.fflags, k.flags) | (if k.flags & EV_ERROR != 0 { EPOLLERR } else { 0 }),
            u64_: k.ident as u64,
        };
    }
    n
}

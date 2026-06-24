//! Bionic ↔ host conversion for libc-shim.
//! On x86_64 Linux the only layout difference is `addrinfo` pointer field ordering.
#![allow(non_snake_case)]

use std::ffi::{c_char, c_int, c_void, CStr};
use crate::types::*;

// ── addrinfo ──

/// Convert bionic ai_flags enum → glibc int flags.
fn to_host_ai_flags(flags: i32) -> i32 {
    let mut ret = 0i32;
    if flags & 1 != 0 { ret |= libc::AI_PASSIVE; }
    if flags & 2 != 0 { ret |= libc::AI_CANONNAME; }
    if flags & 4 != 0 { ret |= libc::AI_NUMERICHOST; }
    ret
}

fn from_host_ai_flags(flags: i32) -> i32 {
    let mut ret = 0i32;
    if flags & libc::AI_PASSIVE != 0 { ret |= 1; }
    if flags & libc::AI_CANONNAME != 0 { ret |= 2; }
    if flags & libc::AI_NUMERICHOST != 0 { ret |= 4; }
    ret
}

pub unsafe fn getaddrinfo_impl(
    node: *const c_char,
    service: *const c_char,
    hints: *const bionic_addrinfo,
    res: *mut *mut bionic_addrinfo,
) -> c_int {
    let host_hints = if !hints.is_null() {
        let h = libc::malloc(std::mem::size_of::<libc::addrinfo>()) as *mut libc::addrinfo;
        if h.is_null() { return libc::ENOMEM; }
        std::ptr::write(h, libc::addrinfo {
            ai_flags: to_host_ai_flags((*hints).ai_flags),
            ai_family: (*hints).ai_family,
            ai_socktype: (*hints).ai_socktype,
            ai_protocol: (*hints).ai_protocol,
            ai_addrlen: (*hints).ai_addrlen,
            ai_addr: (*hints).ai_addr as *mut libc::sockaddr,
            ai_canonname: (*hints).ai_canonname,
            ai_next: std::ptr::null_mut(),
        });
        h
    } else {
        std::ptr::null_mut()
    };

    let mut host_res: *mut libc::addrinfo = std::ptr::null_mut();
    let ret = libc::getaddrinfo(node, service, host_hints, &mut host_res);

    if !host_hints.is_null() { libc::free(host_hints as *mut c_void); }
    if ret != 0 { *res = std::ptr::null_mut(); return ret; }

    // Convert result host→bionic (the two pointer fields are swapped
    // at the byte level).  Deep-copy ai_addr and ai_canonname.
    let mut prev: *mut bionic_addrinfo = std::ptr::null_mut();
    let mut cur = host_res;
    while !cur.is_null() {
        let bio = libc::malloc(std::mem::size_of::<bionic_addrinfo>()) as *mut bionic_addrinfo;
        if bio.is_null() {
            if !prev.is_null() { freeaddrinfo_impl(*res); }
            libc::freeaddrinfo(host_res);
            *res = std::ptr::null_mut();
            return libc::ENOMEM;
        }

        let sock = if !(*cur).ai_addr.is_null() {
            let p = libc::malloc((*cur).ai_addrlen as _) as *mut libc::sockaddr;
            std::ptr::copy_nonoverlapping((*cur).ai_addr as *const u8, p as *mut u8, (*cur).ai_addrlen as _);
            p
        } else { std::ptr::null_mut() };

        let name = if !(*cur).ai_canonname.is_null() {
            libc::strdup((*cur).ai_canonname)
        } else { std::ptr::null_mut() };

        std::ptr::write(bio, bionic_addrinfo {
            ai_flags: from_host_ai_flags((*cur).ai_flags),
            ai_family: (*cur).ai_family,
            ai_socktype: (*cur).ai_socktype,
            ai_protocol: (*cur).ai_protocol,
            ai_addrlen: (*cur).ai_addrlen,
            ai_canonname: name,
            ai_addr: sock as *mut c_void,
            ai_next: std::ptr::null_mut(),
        });

        if prev.is_null() { *res = bio; } else { (*prev).ai_next = bio; }
        prev = bio;
        cur = (*cur).ai_next;
    }

    libc::freeaddrinfo(host_res);
    ret
}

pub unsafe fn freeaddrinfo_impl(ai: *mut bionic_addrinfo) {
    let mut cur = ai;
    while !cur.is_null() {
        if !(*cur).ai_canonname.is_null() { libc::free((*cur).ai_canonname as *mut c_void); }
        if !(*cur).ai_addr.is_null() { libc::free((*cur).ai_addr as *mut c_void); }
        let next = (*cur).ai_next;
        libc::free(cur as *mut c_void);
        cur = next;
    }
}

pub unsafe fn getnameinfo_impl(
    addr: *const libc::sockaddr,
    addrlen: u32,
    host: *mut c_char,
    hostlen: u32,
    serv: *mut c_char,
    servlen: u32,
    flags: i32,
) -> c_int {
    // sockaddr layout is identical on x86_64 — just cast and forward.
    libc::getnameinfo(addr, addrlen as _,
                      host, hostlen as _, serv, servlen as _, flags)
}

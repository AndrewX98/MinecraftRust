//! bionic pthread compatibility layer.
//! bionic's pthread types are fixed-size int32 arrays that wrap heap-allocated
//! glibc pthread objects.  The first 4 bytes store an init-value tag; the
//! remaining bytes store a pointer to the real glibc object at offset 8 (LP64).
#![allow(non_camel_case_types, dead_code)]

use std::ffi::c_void;
use std::sync::atomic::{AtomicUsize, Ordering};
use crate::types::*;

// ── payload pointer helpers ──
// The payload pointer is stored at the first 8-byte-aligned address
// strictly >= base + sizeof(int32_t).  This matches the C++ shim's
// get_payload_pointer() which does:
//   v = (uintptr_t)p + header_size + pointer_alignment - 1;
//   v &= -pointer_alignment;
// On LP64: header_size=4, pointer_alignment=8 → v = (base + 11) & ~7.
// This ensures the first 4 bytes (the static init value) are never
// overwritten even when the struct is at a 4-aligned address.

fn payload_addr<T>(obj: *const T) -> *mut AtomicUsize {
    let base = obj as usize;
    // C++ equivalent: (base + sizeof(int32_t) + alignof(atomic_uintptr_t) - 1) & -alignof(atomic_uintptr_t)
    let addr = (base + 11) & !7;
    addr as *mut AtomicUsize
}

fn load_payload<T>(obj: *const T) -> usize {
    let p = payload_addr(obj);
    unsafe { (*p).load(Ordering::Relaxed) }
}

fn store_payload<T>(obj: *const T, val: usize) {
    let p = payload_addr(obj);
    unsafe { (*p).store(val, Ordering::Release); }
}

fn try_init_payload<T>(obj: *const T, val: usize) -> bool {
    let p = payload_addr(obj);
    unsafe {
        (*p).compare_exchange(0, val, Ordering::Acquire, Ordering::Relaxed).is_ok()
    }
}

fn is_initialized(v: usize) -> bool { v != 0 }

const MUTEX_INIT_NORMAL: i32 = 0;
const MUTEX_INIT_RECURSIVE: i32 = 0x4000;
const MUTEX_INIT_ERRORCHECK: i32 = 0x8000;

fn to_host_mutex(m: *const bionic_pthread_mutex_t) -> *mut libc::pthread_mutex_t {
    let v = load_payload(m);
    if is_initialized(v) {
        return v as *mut libc::pthread_mutex_t;
    }
    let init_val = unsafe { (*m).data[0] };
    let host = unsafe { libc::malloc(std::mem::size_of::<libc::pthread_mutex_t>()) as *mut libc::pthread_mutex_t };
    if host.is_null() { return std::ptr::null_mut(); }
    let mut attr: libc::pthread_mutexattr_t = unsafe { std::mem::zeroed() };
    unsafe { libc::pthread_mutexattr_init(&mut attr) };
    let kind = match init_val {
        MUTEX_INIT_RECURSIVE => libc::PTHREAD_MUTEX_RECURSIVE,
        MUTEX_INIT_ERRORCHECK => libc::PTHREAD_MUTEX_ERRORCHECK,
        _ => libc::PTHREAD_MUTEX_NORMAL,
    };
    unsafe { libc::pthread_mutexattr_settype(&mut attr, kind) };
    let ret = unsafe { libc::pthread_mutex_init(host, &attr) };
    unsafe { libc::pthread_mutexattr_destroy(&mut attr) };
    if ret != 0 { unsafe { libc::free(host as *mut c_void) }; return std::ptr::null_mut(); }
    if try_init_payload(m, host as usize) {
        host
    } else {
        let existing = load_payload(m) as *mut libc::pthread_mutex_t;
        unsafe { libc::pthread_mutex_destroy(host); libc::free(host as *mut c_void); }
        existing
    }
}

fn to_host_cond(c: *const bionic_pthread_cond_t) -> *mut libc::pthread_cond_t {
    let v = load_payload(c);
    if is_initialized(v) { return v as *mut libc::pthread_cond_t; }
    let host = unsafe { libc::malloc(std::mem::size_of::<libc::pthread_cond_t>()) as *mut libc::pthread_cond_t };
    if host.is_null() { return std::ptr::null_mut(); }
    let ret = unsafe { libc::pthread_cond_init(host, std::ptr::null()) };
    if ret != 0 { unsafe { libc::free(host as *mut c_void) }; return std::ptr::null_mut(); }
    if try_init_payload(c, host as usize) {
        host
    } else {
        let existing = load_payload(c) as *mut libc::pthread_cond_t;
        unsafe { libc::pthread_cond_destroy(host); libc::free(host as *mut c_void); }
        existing
    }
}

fn to_host_rwlock(r: *const bionic_pthread_rwlock_t) -> *mut libc::pthread_rwlock_t {
    let v = load_payload(r);
    if is_initialized(v) { return v as *mut libc::pthread_rwlock_t; }
    let host = unsafe { libc::malloc(std::mem::size_of::<libc::pthread_rwlock_t>()) as *mut libc::pthread_rwlock_t };
    if host.is_null() { return std::ptr::null_mut(); }
    let ret = unsafe { libc::pthread_rwlock_init(host, std::ptr::null()) };
    if ret != 0 { unsafe { libc::free(host as *mut c_void) }; return std::ptr::null_mut(); }
    if try_init_payload(r, host as usize) {
        host
    } else {
        let existing = load_payload(r) as *mut libc::pthread_rwlock_t;
        unsafe { libc::pthread_rwlock_destroy(host); libc::free(host as *mut c_void); }
        existing
    }
}

// ── clock type conversion ──

#[repr(u32)]
pub enum clock_type {
    REALTIME = 0,
    MONOTONIC = 1,
    BOOTTIME = 7,
}

pub fn to_host_clock_type(ct: clock_type) -> libc::clockid_t {
    match ct {
        clock_type::REALTIME => libc::CLOCK_REALTIME,
        clock_type::MONOTONIC => libc::CLOCK_MONOTONIC,
        clock_type::BOOTTIME => libc::CLOCK_BOOTTIME,
    }
}

// ── sched_policy ──

pub fn to_host_sched_policy(sp: i32) -> i32 {
    match sp {
        0 => libc::SCHED_OTHER,
        _ => libc::SCHED_OTHER,
    }
}

pub fn from_host_sched_policy(sp: i32) -> i32 {
    match sp {
        libc::SCHED_OTHER => 0,
        _ => -1,
    }
}

// ── public API: thread management ──

pub unsafe extern "C" fn pthread_create(
    thread: *mut libc::pthread_t,
    attr: *const bionic_pthread_attr_t,
    start: Option<unsafe extern "C" fn(*mut c_void) -> *mut c_void>,
    arg: *mut c_void,
) -> i32 {
    let mut host_attr: libc::pthread_attr_t = std::mem::zeroed();
    libc::pthread_attr_init(&mut host_attr);
    if !attr.is_null() {
        let b = &*attr;
        libc::pthread_attr_setdetachstate(&mut host_attr, if (b.flags & 1) != 0 { libc::PTHREAD_CREATE_DETACHED } else { libc::PTHREAD_CREATE_JOINABLE });
        if b.stack_size > 0 { libc::pthread_attr_setstacksize(&mut host_attr, b.stack_size); }
        if b.sched_priority != 0 {
            let mut param: libc::sched_param = std::mem::zeroed();
            param.sched_priority = b.sched_priority;
            libc::pthread_attr_setschedparam(&mut host_attr, &param);
        }
    }
    let start_fn = match start {
        Some(f) => std::mem::transmute::<unsafe extern "C" fn(*mut c_void) -> *mut c_void, extern "C" fn(*mut c_void) -> *mut c_void>(f),
        None => return libc::EINVAL,
    };
    let ret = libc::pthread_create(thread, &host_attr, start_fn, arg);
    libc::pthread_attr_destroy(&mut host_attr);
    ret as i32
}

pub unsafe extern "C" fn pthread_join(thread: libc::pthread_t, retval: *mut *mut c_void) -> i32 {
    libc::pthread_join(thread, retval)
}

pub unsafe extern "C" fn pthread_detach(thread: libc::pthread_t) -> i32 {
    libc::pthread_detach(thread)
}

pub unsafe extern "C" fn pthread_kill(thread: libc::pthread_t, sig: i32) -> i32 {
    libc::pthread_kill(thread, sig)
}

pub unsafe extern "C" fn pthread_self() -> libc::pthread_t {
    libc::pthread_self()
}

pub unsafe extern "C" fn pthread_equal(t1: libc::pthread_t, t2: libc::pthread_t) -> i32 {
    libc::pthread_equal(t1, t2)
}

pub unsafe extern "C" fn pthread_atfork(
    prepare: Option<unsafe extern "C" fn()>,
    parent: Option<unsafe extern "C" fn()>,
    child: Option<unsafe extern "C" fn()>,
) -> i32 {
    libc::pthread_atfork(prepare, parent, child)
}

// ── schedparam ──

pub unsafe extern "C" fn pthread_setschedparam(
    thread: libc::pthread_t,
    policy: i32,
    param: *const bionic_sched_param,
) -> i32 {
    let hpolicy = to_host_sched_policy(policy);
    let mut hparam: libc::sched_param = std::mem::zeroed();
    hparam.sched_priority = (*param).sched_priority;
    libc::pthread_setschedparam(thread, hpolicy, &hparam)
}

pub unsafe extern "C" fn pthread_getschedparam(
    thread: libc::pthread_t,
    policy: *mut i32,
    param: *mut bionic_sched_param,
) -> i32 {
    let mut hpolicy: i32 = 0;
    let mut hparam: libc::sched_param = std::mem::zeroed();
    let ret = libc::pthread_getschedparam(thread, &mut hpolicy, &mut hparam);
    if ret == 0 {
        *policy = from_host_sched_policy(hpolicy);
        (*param).sched_priority = hparam.sched_priority;
    }
    ret as i32
}

pub unsafe extern "C" fn pthread_getattr_np(thread: libc::pthread_t, attr: *mut bionic_pthread_attr_t) -> i32 {
    let mut host_attr: libc::pthread_attr_t = std::mem::zeroed();
    let ret = libc::pthread_getattr_np(thread, &mut host_attr);
    if ret != 0 { return ret as i32; }
    let mut detach: i32 = 0;
    extern "C" { fn pthread_attr_getdetachstate(attr: *const libc::pthread_attr_t, detachstate: *mut i32) -> i32; }
    pthread_attr_getdetachstate(&host_attr, &mut detach);
    pthread_attr_setdetachstate(attr, detach);
    let mut hparam: libc::sched_param = std::mem::zeroed();
    libc::pthread_attr_getschedparam(&host_attr, &mut hparam);
    (*attr).sched_priority = hparam.sched_priority;
    let mut stackaddr: *mut c_void = std::ptr::null_mut();
    let mut stacksize: usize = 0;
    libc::pthread_attr_getstack(&host_attr, &mut stackaddr, &mut stacksize);
    (*attr).stack_base = stackaddr;
    (*attr).stack_size = stacksize;
    libc::pthread_attr_destroy(&mut host_attr);
    0
}

// ── attr ──

pub unsafe extern "C" fn pthread_attr_init(attr: *mut bionic_pthread_attr_t) -> i32 {
    std::ptr::write(attr, bionic_pthread_attr_t {
        flags: 0,
        stack_base: std::ptr::null_mut(), stack_size: 0, guard_size: 0,
        sched_policy: 0, sched_priority: 0, __padding: [0; 4],
    });
    0
}

pub unsafe extern "C" fn pthread_attr_destroy(_attr: *mut bionic_pthread_attr_t) -> i32 { 0 }

pub unsafe extern "C" fn pthread_attr_setdetachstate(attr: *mut bionic_pthread_attr_t, val: i32) -> i32 {
    if val != 0 && val != 1 { return libc::EINVAL; }
    if val != 0 {
        (*attr).flags |= 1;
    } else {
        (*attr).flags &= !1;
    }
    0
}

pub unsafe extern "C" fn pthread_attr_getdetachstate(attr: *const bionic_pthread_attr_t, val: *mut i32) -> i32 {
    *val = ((*attr).flags & 1) as i32;
    0
}

pub unsafe extern "C" fn pthread_attr_setschedparam(attr: *mut bionic_pthread_attr_t, param: *const bionic_sched_param) -> i32 {
    (*attr).sched_priority = (*param).sched_priority;
    0
}

pub unsafe extern "C" fn pthread_attr_getschedparam(attr: *const bionic_pthread_attr_t, param: *mut bionic_sched_param) -> i32 {
    (*param).sched_priority = (*attr).sched_priority;
    0
}

pub unsafe extern "C" fn pthread_attr_setstacksize(attr: *mut bionic_pthread_attr_t, sz: usize) -> i32 {
    (*attr).stack_size = sz;
    0
}

pub unsafe extern "C" fn pthread_attr_getstack(attr: *const bionic_pthread_attr_t, stackaddr: *mut *mut c_void, stacksize: *mut usize) -> i32 {
    *stackaddr = (*attr).stack_base;
    *stacksize = (*attr).stack_size;
    0
}

pub unsafe extern "C" fn pthread_attr_getstacksize(attr: *const bionic_pthread_attr_t, val: *mut usize) -> i32 {
    *val = (*attr).stack_size;
    0
}

pub unsafe extern "C" fn pthread_setname_np(thread: libc::pthread_t, name: *const std::ffi::c_char) -> i32 {
    libc::pthread_setname_np(thread, name)
}

// ── mutex ──

pub unsafe extern "C" fn pthread_mutex_init(m: *mut bionic_pthread_mutex_t, attr: *const bionic_pthread_mutexattr_t) -> i32 {
    let host = libc::malloc(std::mem::size_of::<libc::pthread_mutex_t>()) as *mut libc::pthread_mutex_t;
    if host.is_null() { return libc::ENOMEM; }
    let mut host_attr: libc::pthread_mutexattr_t = std::mem::zeroed();
    libc::pthread_mutexattr_init(&mut host_attr);
    if !attr.is_null() {
        let kind = match (*attr).type_ {
            1 => libc::PTHREAD_MUTEX_RECURSIVE,
            2 => libc::PTHREAD_MUTEX_ERRORCHECK,
            _ => libc::PTHREAD_MUTEX_NORMAL,
        };
        libc::pthread_mutexattr_settype(&mut host_attr, kind);
    }
    let ret = libc::pthread_mutex_init(host, &host_attr);
    libc::pthread_mutexattr_destroy(&mut host_attr);
    if ret != 0 { libc::free(host as *mut c_void); return ret as i32; }
    store_payload(m, host as usize);
    0
}

pub unsafe extern "C" fn pthread_mutex_destroy(m: *mut bionic_pthread_mutex_t) -> i32 {
    let v = load_payload(m);
    if !is_initialized(v) { return 0; }
    let ret = libc::pthread_mutex_destroy(v as *mut libc::pthread_mutex_t);
    libc::free(v as *mut c_void);
    store_payload(m, 0);
    ret as i32
}

pub unsafe extern "C" fn pthread_mutex_lock(m: *mut bionic_pthread_mutex_t) -> i32 {
    libc::pthread_mutex_lock(to_host_mutex(m))
}

pub unsafe extern "C" fn pthread_mutex_unlock(m: *mut bionic_pthread_mutex_t) -> i32 {
    libc::pthread_mutex_unlock(to_host_mutex(m))
}

pub unsafe extern "C" fn pthread_mutex_trylock(m: *mut bionic_pthread_mutex_t) -> i32 {
    libc::pthread_mutex_trylock(to_host_mutex(m))
}

pub unsafe extern "C" fn pthread_mutexattr_init(attr: *mut bionic_pthread_mutexattr_t) -> i32 {
    std::ptr::write(attr, bionic_pthread_mutexattr_t { type_: 0 });
    0
}

pub unsafe extern "C" fn pthread_mutexattr_destroy(_attr: *mut bionic_pthread_mutexattr_t) -> i32 { 0 }

pub unsafe extern "C" fn pthread_mutexattr_settype(attr: *mut bionic_pthread_mutexattr_t, t: i32) -> i32 {
    if t < 0 || t > 2 { return libc::EINVAL; }
    (*attr).type_ = t as u32;
    0
}

pub unsafe extern "C" fn pthread_mutexattr_gettype(attr: *const bionic_pthread_mutexattr_t, t: *mut i32) -> i32 {
    *t = (*attr).type_ as i32;
    0
}

// ── cond ──

pub unsafe extern "C" fn pthread_cond_init(c: *mut bionic_pthread_cond_t, attr: *const bionic_pthread_condattr_t) -> i32 {
    let host = libc::malloc(std::mem::size_of::<libc::pthread_cond_t>()) as *mut libc::pthread_cond_t;
    if host.is_null() { return libc::ENOMEM; }
    let ret = if !attr.is_null() && attr.read().clock != 0 {
        let mut host_attr: libc::pthread_condattr_t = std::mem::zeroed();
        libc::pthread_condattr_init(&mut host_attr);
        let ct = if attr.read().clock == 7 { libc::CLOCK_BOOTTIME } else if attr.read().clock == 1 { libc::CLOCK_MONOTONIC } else { libc::CLOCK_REALTIME };
        libc::pthread_condattr_setclock(&mut host_attr, ct);
        let r = libc::pthread_cond_init(host, &host_attr);
        libc::pthread_condattr_destroy(&mut host_attr);
        r
    } else {
        libc::pthread_cond_init(host, std::ptr::null())
    };
    if ret != 0 { libc::free(host as *mut c_void); return ret as i32; }
    store_payload(c, host as usize);
    0
}

pub unsafe extern "C" fn pthread_cond_destroy(c: *mut bionic_pthread_cond_t) -> i32 {
    let v = load_payload(c);
    if !is_initialized(v) { return 0; }
    let ret = libc::pthread_cond_destroy(v as *mut libc::pthread_cond_t);
    libc::free(v as *mut c_void);
    store_payload(c, 0);
    ret as i32
}

pub unsafe extern "C" fn pthread_cond_wait(c: *mut bionic_pthread_cond_t, m: *mut bionic_pthread_mutex_t) -> i32 {
    libc::pthread_cond_wait(to_host_cond(c), to_host_mutex(m))
}

pub unsafe extern "C" fn pthread_cond_broadcast(c: *mut bionic_pthread_cond_t) -> i32 {
    libc::pthread_cond_broadcast(to_host_cond(c))
}

pub unsafe extern "C" fn pthread_cond_signal(c: *mut bionic_pthread_cond_t) -> i32 {
    libc::pthread_cond_signal(to_host_cond(c))
}

pub unsafe extern "C" fn pthread_cond_timedwait(c: *mut bionic_pthread_cond_t, m: *mut bionic_pthread_mutex_t, ts: *const libc::timespec) -> i32 {
    libc::pthread_cond_timedwait(to_host_cond(c), to_host_mutex(m), ts)
}

pub unsafe extern "C" fn pthread_condattr_init(attr: *mut bionic_pthread_condattr_t) -> i32 {
    std::ptr::write(attr, bionic_pthread_condattr_t { shared: 0, clock: 1 });
    0
}

pub unsafe extern "C" fn pthread_condattr_destroy(_attr: *mut bionic_pthread_condattr_t) -> i32 { 0 }

pub unsafe extern "C" fn pthread_condattr_setclock(attr: *mut bionic_pthread_condattr_t, clock: i32) -> i32 {
    if clock != 0 && clock != 1 && clock != 7 { return libc::EINVAL; }
    (*attr).clock = clock as u32;
    0
}

pub unsafe extern "C" fn pthread_condattr_getclock(attr: *const bionic_pthread_condattr_t, clock: *mut i32) -> i32 {
    *clock = (*attr).clock as i32;
    0
}

// ── rwlock ──

pub unsafe extern "C" fn pthread_rwlock_init(r: *mut bionic_pthread_rwlock_t, attr: *const libc::pthread_rwlockattr_t) -> i32 {
    let host = libc::malloc(std::mem::size_of::<libc::pthread_rwlock_t>()) as *mut libc::pthread_rwlock_t;
    if host.is_null() { return libc::ENOMEM; }
    let ret = libc::pthread_rwlock_init(host, attr);
    if ret != 0 { libc::free(host as *mut c_void); return ret as i32; }
    store_payload(r, host as usize);
    0
}

pub unsafe extern "C" fn pthread_rwlock_destroy(r: *mut bionic_pthread_rwlock_t) -> i32 {
    let v = load_payload(r);
    if !is_initialized(v) { return 0; }
    let ret = libc::pthread_rwlock_destroy(v as *mut libc::pthread_rwlock_t);
    libc::free(v as *mut c_void);
    store_payload(r, 0);
    ret as i32
}

pub unsafe extern "C" fn pthread_rwlock_rdlock(r: *mut bionic_pthread_rwlock_t) -> i32 {
    libc::pthread_rwlock_rdlock(to_host_rwlock(r))
}

pub unsafe extern "C" fn pthread_rwlock_wrlock(r: *mut bionic_pthread_rwlock_t) -> i32 {
    libc::pthread_rwlock_wrlock(to_host_rwlock(r))
}

pub unsafe extern "C" fn pthread_rwlock_tryrdlock(r: *mut bionic_pthread_rwlock_t) -> i32 {
    libc::pthread_rwlock_tryrdlock(to_host_rwlock(r))
}

pub unsafe extern "C" fn pthread_rwlock_trywrlock(r: *mut bionic_pthread_rwlock_t) -> i32 {
    libc::pthread_rwlock_trywrlock(to_host_rwlock(r))
}

pub unsafe extern "C" fn pthread_rwlock_unlock(r: *mut bionic_pthread_rwlock_t) -> i32 {
    libc::pthread_rwlock_unlock(to_host_rwlock(r))
}

// ── keys ──

pub unsafe extern "C" fn pthread_key_create(key: *mut bionic_pthread_key_t, dtor: Option<unsafe extern "C" fn(*mut c_void)>) -> i32 {
    let mut host_key: libc::pthread_key_t = 0;
    let ret = libc::pthread_key_create(&mut host_key, dtor);
    *key = host_key as bionic_pthread_key_t;
    ret as i32
}

pub unsafe extern "C" fn pthread_key_delete(key: bionic_pthread_key_t) -> i32 {
    libc::pthread_key_delete(key as libc::pthread_key_t)
}

pub unsafe extern "C" fn pthread_setspecific(key: bionic_pthread_key_t, val: *const c_void) -> i32 {
    libc::pthread_setspecific(key as libc::pthread_key_t, val)
}

pub unsafe extern "C" fn pthread_getspecific(key: bionic_pthread_key_t) -> *mut c_void {
    libc::pthread_getspecific(key as libc::pthread_key_t)
}

// ── once ──

pub unsafe extern "C" fn pthread_once(control: *mut bionic_pthread_once_t, routine: Option<unsafe extern "C" fn()>) -> i32 {
    let ctrl = &*(control as *const std::sync::atomic::AtomicI32);
    if ctrl.compare_exchange(0, 1, Ordering::Release, Ordering::Acquire).is_ok() {
        if let Some(f) = routine { f(); }
        ctrl.store(2, Ordering::Release);
        return 0;
    }
    if ctrl.load(Ordering::Acquire) == 2 { return 0; }
    while ctrl.load(Ordering::Acquire) == 1 { std::thread::yield_now(); }
    0
}

// ── cleanup ──

pub unsafe extern "C" fn __pthread_cleanup_push(_c: *mut c_void, _cb: Option<unsafe extern "C" fn(*mut c_void)>, _arg: *mut c_void) {
}

pub unsafe extern "C" fn __pthread_cleanup_pop(_c: *mut c_void, _execute: i32) {
}

pub unsafe extern "C" fn pthread_gettid_np(thread: libc::pthread_t) -> i32 {
    crate::misc::gettid()
}

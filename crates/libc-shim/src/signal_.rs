#![allow(non_camel_case_types, unused)]

pub unsafe extern "C" fn signal(sig: i32, handler: *mut std::ffi::c_void) -> *mut std::ffi::c_void {
    extern "C" { fn signal(sig: i32, handler: usize) -> usize; }
    signal(sig, handler as usize) as *mut std::ffi::c_void
}
pub unsafe extern "C" fn raise(sig: i32) -> i32 { libc::raise(sig) }
pub unsafe extern "C" fn sigaction(sig: i32, act: *const libc::sigaction, oact: *mut libc::sigaction) -> i32 {
    libc::sigaction(sig, act, oact)
}
pub unsafe extern "C" fn sigprocmask(how: i32, set: *const libc::sigset_t, oset: *mut libc::sigset_t) -> i32 {
    libc::sigprocmask(how, set, oset)
}
pub unsafe extern "C" fn sigemptyset(set: *mut libc::sigset_t) -> i32 { libc::sigemptyset(set) }
pub unsafe extern "C" fn sigfillset(set: *mut libc::sigset_t) -> i32 { libc::sigfillset(set) }
pub unsafe extern "C" fn sigaddset(set: *mut libc::sigset_t, signo: i32) -> i32 { libc::sigaddset(set, signo) }
pub unsafe extern "C" fn sigdelset(set: *mut libc::sigset_t, signo: i32) -> i32 { libc::sigdelset(set, signo) }
pub unsafe extern "C" fn bsd_signal(sig: i32, func: Option<unsafe extern "C" fn(i32)>) -> Option<unsafe extern "C" fn(i32)> {
    let old = libc::signal(sig, std::mem::transmute::<Option<unsafe extern "C" fn(i32)>, usize>(func));
    std::mem::transmute::<usize, Option<unsafe extern "C" fn(i32)>>(old)
}
pub unsafe extern "C" fn pthread_sigmask(how: i32, set: *const libc::sigset_t, oldset: *mut libc::sigset_t) -> i32 {
    libc::pthread_sigmask(how, set, oldset)
}
pub unsafe extern "C" fn kill(pid: i32, sig: i32) -> i32 { libc::kill(pid, sig) }
pub unsafe extern "C" fn killpg(pgrp: i32, sig: i32) -> i32 { libc::killpg(pgrp, sig) }

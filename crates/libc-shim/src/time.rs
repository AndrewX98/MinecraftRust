#![allow(non_camel_case_types, unused)]

use std::ffi::c_char;

extern "C" {
    #[link_name = "clock"]
    fn libc_clock() -> libc::clock_t;
    #[link_name = "asctime"]
    fn libc_asctime(tm: *const libc::tm) -> *mut c_char;
    #[link_name = "ctime"]
    fn libc_ctime(t: *const libc::time_t) -> *mut c_char;
    #[link_name = "tzset"]
    fn libc_tzset();
}

#[no_mangle]
pub static mut tzname: [*mut c_char; 2] = [std::ptr::null_mut(); 2];
#[no_mangle]
pub static mut daylight: i32 = 0;
#[no_mangle]
pub static mut timezone: i64 = 0;

pub unsafe extern "C" fn time(t: *mut libc::time_t) -> libc::time_t { libc::time(t) }
pub unsafe extern "C" fn clock() -> libc::clock_t { libc_clock() }
pub unsafe extern "C" fn difftime(time1: libc::time_t, time0: libc::time_t) -> f64 { libc::difftime(time1, time0) }
pub unsafe extern "C" fn mktime(tm: *mut libc::tm) -> libc::time_t { libc::mktime(tm) }
pub unsafe extern "C" fn gmtime(t: *const libc::time_t) -> *mut libc::tm { libc::gmtime(t) }
pub unsafe extern "C" fn gmtime_r(t: *const libc::time_t, result: *mut libc::tm) -> *mut libc::tm { libc::gmtime_r(t, result) }
pub unsafe extern "C" fn localtime(t: *const libc::time_t) -> *mut libc::tm { libc::localtime(t) }
pub unsafe extern "C" fn localtime_r(t: *const libc::time_t, result: *mut libc::tm) -> *mut libc::tm { libc::localtime_r(t, result) }
pub unsafe extern "C" fn strftime(s: *mut c_char, max: usize, fmt: *const c_char, tm: *const libc::tm) -> usize { libc::strftime(s, max, fmt, tm) }
pub unsafe extern "C" fn strptime(s: *const c_char, fmt: *const c_char, tm: *mut libc::tm) -> *mut c_char { libc::strptime(s, fmt, tm) }
pub unsafe extern "C" fn asctime(tm: *const libc::tm) -> *mut c_char { libc_asctime(tm) }
pub unsafe extern "C" fn ctime(t: *const libc::time_t) -> *mut c_char { libc_ctime(t) }
pub unsafe extern "C" fn asctime_r(tm: *const libc::tm, buf: *mut c_char) -> *mut c_char { libc::asctime_r(tm, buf) }
pub unsafe extern "C" fn ctime_r(t: *const libc::time_t, buf: *mut c_char) -> *mut c_char { libc::ctime_r(t, buf) }
pub unsafe extern "C" fn tzset() { libc_tzset(); }
pub unsafe extern "C" fn nanosleep(req: *const libc::timespec, rem: *mut libc::timespec) -> i32 { libc::nanosleep(req, rem) }
pub unsafe extern "C" fn gettimeofday(tv: *mut libc::timeval, tz: *mut std::ffi::c_void) -> i32 {
    libc::gettimeofday(tv, tz as *mut libc::timezone)
}
pub unsafe extern "C" fn clock_gettime(clock_id: u32, tp: *mut libc::timespec) -> i32 { libc::clock_gettime(clock_id as libc::clockid_t, tp) }
pub unsafe extern "C" fn clock_getres(clock_id: u32, tp: *mut libc::timespec) -> i32 { libc::clock_getres(clock_id as libc::clockid_t, tp) }

pub unsafe extern "C" fn strftime_l(s: *mut c_char, max: usize, fmt: *const c_char, tm: *const libc::tm, _locale: *mut std::ffi::c_void) -> usize { libc::strftime(s, max, fmt, tm) }
pub unsafe extern "C" fn strptime_l(s: *const c_char, fmt: *const c_char, tm: *mut libc::tm, _locale: *mut std::ffi::c_void) -> *mut c_char { libc::strptime(s, fmt, tm) }

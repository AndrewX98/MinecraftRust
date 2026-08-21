#![allow(non_camel_case_types, unused)]

use std::ffi::{c_char, CStr};

pub unsafe extern "C" fn __assert(file: *const c_char, line: i32, msg: *const c_char) {
    let file = CStr::from_ptr(file).to_str().unwrap_or("?");
    let msg = CStr::from_ptr(msg).to_str().unwrap_or("?");
    eprintln!("assert failed: {}:{}: {}", file, line, msg);
    std::process::abort();
}

pub unsafe extern "C" fn __assert2(file: *const c_char, line: i32, function: *const c_char, msg: *const c_char) {
    let file = CStr::from_ptr(file).to_str().unwrap_or("?");
    let function = CStr::from_ptr(function).to_str().unwrap_or("?");
    let msg = CStr::from_ptr(msg).to_str().unwrap_or("?");
    eprintln!("assert failed: {}:{} {}: {}", file, line, function, msg);
    std::process::abort();
}

pub unsafe extern "C" fn __stack_chk_fail() {
    eprintln!("stack corruption detected");
    std::process::abort();
}

#[no_mangle]
pub static __stack_chk_guard: usize = 0;

pub unsafe extern "C" fn android_set_abort_message(msg: *const c_char) {
    let msg = CStr::from_ptr(msg).to_str().unwrap_or("?");
    eprintln!("abort message: {}", msg);
}

pub unsafe extern "C" fn abort() {
    #[cfg(not(target_os = "macos"))]
    unsafe fn cxa_parts() -> (Option<unsafe extern "C" fn() -> *const core::ffi::c_void>, Option<unsafe extern "C" fn(*const c_char, *mut c_char, *mut usize, *mut i32) -> *mut c_char>) {
        extern "C" {
            fn __cxa_current_exception_type() -> *const core::ffi::c_void;
            fn __cxa_demangle(name: *const c_char, out: *mut c_char, len: *mut usize, status: *mut i32) -> *mut c_char;
        }
        (Some(__cxa_current_exception_type), Some(__cxa_demangle))
    }
    // libc++abi symbols are not linked into a plain Rust binary on macOS;
    // abort diagnostics degrade gracefully instead
    #[cfg(target_os = "macos")]
    unsafe fn cxa_parts() -> (Option<unsafe extern "C" fn() -> *const core::ffi::c_void>, Option<unsafe extern "C" fn(*const c_char, *mut c_char, *mut usize, *mut i32) -> *mut c_char>) {
        (None, None)
    }
    let (current_exception_type, cxa_demangle) = cxa_parts();
    let ex_type = match current_exception_type { Some(f) => f(), None => std::ptr::null() };
    if !ex_type.is_null() {
        // std::type_info layout: [vtable_ptr(8), __type_name(8)]
        let type_name_ptr = (ex_type as *const *const c_char).add(1);
        let mangled = *type_name_ptr;
        if !mangled.is_null() {
            let mangled_str = CStr::from_ptr(mangled).to_str().unwrap_or("?");
            eprintln!("uncaught exception type: {}", mangled_str);
            if let Some(demangle) = cxa_demangle {
                let demangled = demangle(mangled, std::ptr::null_mut(), std::ptr::null_mut(), std::ptr::null_mut());
                if !demangled.is_null() {
                    let demangled_str = CStr::from_ptr(demangled).to_str().unwrap_or("?");
                    eprintln!("  demangled: {}", demangled_str);
                    libc::free(demangled as *mut libc::c_void);
                }
            }
        }
    } else {
        eprintln!("abort called (no active C++ exception)");
    }
    extern "C" {
        fn backtrace(buf: *mut *mut core::ffi::c_void, size: i32) -> i32;
        fn backtrace_symbols_fd(buf: *mut *mut core::ffi::c_void, size: i32, fd: i32);
    }
    let mut buf: [*mut core::ffi::c_void; 32] = [std::ptr::null_mut(); 32];
    let n = backtrace(buf.as_mut_ptr(), 32);
    backtrace_symbols_fd(buf.as_mut_ptr(), n, 2);
    std::process::abort();
}

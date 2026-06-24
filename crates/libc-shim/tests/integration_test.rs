use std::ffi::{c_char, c_void, CStr, CString};

// Functions with #[no_mangle] — accessible by symbol name
extern "C" {
    fn get_shimmed_symbols_len() -> usize;
    fn get_shimmed_symbols_fill(buf: *mut ShimmedSymbol);
}

// Functions without #[no_mangle] — use crate's public API
use libc_shim::errno::__errno;
use libc_shim::errno::__set_errno;

#[repr(C)]
#[derive(Clone)]
struct ShimmedSymbol {
    name: *const c_char,
    value: *mut c_void,
}

#[test]
fn test_shimmed_symbols_len() {
    let len = unsafe { get_shimmed_symbols_len() };
    assert!(len > 0);
    eprintln!("shimmed symbols count: {}", len);
}

#[test]
fn test_shimmed_symbols_fill_has_expected() {
    let len = unsafe { get_shimmed_symbols_len() };
    let mut buf = vec![ShimmedSymbol{name: std::ptr::null(), value: std::ptr::null_mut()}; len];
    unsafe { get_shimmed_symbols_fill(buf.as_mut_ptr()) };
    for s in &buf {
        let name = unsafe { CStr::from_ptr(s.name) }.to_str().unwrap_or("?");
        assert!(!s.value.is_null(), "symbol {} has null value", name);
    }
    let has_malloc = buf.iter().any(|s| {
        let name = unsafe { CStr::from_ptr(s.name) }.to_str().unwrap_or("");
        name == "malloc"
    });
    assert!(has_malloc);
    let has_open = buf.iter().any(|s| {
        let name = unsafe { CStr::from_ptr(s.name) }.to_str().unwrap_or("");
        name == "open"
    });
    assert!(has_open);
}

#[test]
fn test_errno() {
    unsafe {
        let ptr = __errno();
        assert!(!ptr.is_null());
        let old = *ptr;
        __set_errno(42);
        assert_eq!(*ptr, 42);
        __set_errno(old);
    }
}

#[test]
fn test_call_strlen_through_symbol_table() {
    let len = unsafe { get_shimmed_symbols_len() };
    let mut buf = vec![ShimmedSymbol{name: std::ptr::null(), value: std::ptr::null_mut()}; len];
    unsafe { get_shimmed_symbols_fill(buf.as_mut_ptr()) };
    let strlen_fn = buf.iter().find(|s| {
        let name = unsafe { CStr::from_ptr(s.name) }.to_str().unwrap_or("");
        name == "strlen"
    }).expect("strlen should be in symbol table");
    let f: unsafe extern "C" fn(*const c_char) -> usize = unsafe { std::mem::transmute(strlen_fn.value) };
    let s = CString::new("hello").unwrap();
    let result = unsafe { f(s.as_ptr()) };
    assert_eq!(result, 5);
}

#[test]
fn test_call_open_close_through_symbol_table() {
    let len = unsafe { get_shimmed_symbols_len() };
    let mut buf = vec![ShimmedSymbol{name: std::ptr::null(), value: std::ptr::null_mut()}; len];
    unsafe { get_shimmed_symbols_fill(buf.as_mut_ptr()) };
    let open_fn = buf.iter().find(|s| {
        let name = unsafe { CStr::from_ptr(s.name) }.to_str().unwrap_or("");
        name == "open"
    }).expect("open should be in symbol table");
    let close_fn = buf.iter().find(|s| {
        let name = unsafe { CStr::from_ptr(s.name) }.to_str().unwrap_or("");
        name == "close"
    }).expect("close should be in symbol table");
    let open: unsafe extern "C" fn(*const c_char, i32) -> i32 = unsafe { std::mem::transmute(open_fn.value) };
    let close: unsafe extern "C" fn(i32) -> i32 = unsafe { std::mem::transmute(close_fn.value) };
    let path = CString::new("/dev/null").unwrap();
    let fd = unsafe { open(path.as_ptr(), 0) };
    assert!(fd >= 0);
    let ret = unsafe { close(fd) };
    assert_eq!(ret, 0);
}

#[test]
fn test_call_malloc_free_through_symbol_table() {
    let len = unsafe { get_shimmed_symbols_len() };
    let mut buf = vec![ShimmedSymbol{name: std::ptr::null(), value: std::ptr::null_mut()}; len];
    unsafe { get_shimmed_symbols_fill(buf.as_mut_ptr()) };
    let malloc_fn = buf.iter().find(|s| {
        let name = unsafe { CStr::from_ptr(s.name) }.to_str().unwrap_or("");
        name == "malloc"
    }).expect("malloc should be in symbol table");
    let free_fn = buf.iter().find(|s| {
        let name = unsafe { CStr::from_ptr(s.name) }.to_str().unwrap_or("");
        name == "free"
    }).expect("free should be in symbol table");
    let malloc: unsafe extern "C" fn(usize) -> *mut c_void = unsafe { std::mem::transmute(malloc_fn.value) };
    let free: unsafe extern "C" fn(*mut c_void) = unsafe { std::mem::transmute(free_fn.value) };
    let ptr = unsafe { malloc(128) };
    assert!(!ptr.is_null());
    unsafe { free(ptr) };
}

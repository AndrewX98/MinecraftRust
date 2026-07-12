use std::cell::RefCell;

pub(crate) fn clear_dlwarning() {
    CURRENT_MSG.with(|current_msg| {
        current_msg.borrow_mut().clear();
    });
}

thread_local! {
    static CURRENT_MSG: RefCell<String> = const { RefCell::new(String::new()) };
}

pub fn add_dlwarning(sopath: &str, message: &str, value: Option<&str>) {
    CURRENT_MSG.with(|current_msg| {
        let mut msg = current_msg.borrow_mut();
        if !msg.is_empty() {
            msg.push('\n');
        }
        let base = std::path::Path::new(sopath)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(sopath);
        msg.push_str(&format!("{}: {}", base, message));
        if let Some(v) = value {
            msg.push_str(&format!(" \"{}\"", v));
        }
    });
}

pub fn get_dlwarning(
    obj: *mut std::ffi::c_void,
    f: Option<unsafe extern "C" fn(*mut std::ffi::c_void, *const libc::c_char)>,
) {
    CURRENT_MSG.with(|current_msg| {
        let mut msg = current_msg.borrow_mut();
        if msg.is_empty() {
            if let Some(cb) = f {
                unsafe {
                    cb(obj, std::ptr::null());
                }
            }
        } else {
            let s = std::mem::take(&mut *msg);
            if let Some(cb) = f {
                let cstr = std::ffi::CString::new(s.as_str()).unwrap();
                unsafe {
                    cb(obj, cstr.as_ptr());
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    static CALLED: AtomicBool = AtomicBool::new(false);

    unsafe extern "C" fn check_warning_cb(_obj: *mut std::ffi::c_void, msg: *const libc::c_char) {
        if !msg.is_null() {
            let s = unsafe { std::ffi::CStr::from_ptr(msg) }.to_str().unwrap();
            assert_eq!(s, "libtest.so: test warning");
        }
        CALLED.store(true, Ordering::SeqCst);
    }

    unsafe extern "C" fn null_warning_cb(_obj: *mut std::ffi::c_void, msg: *const libc::c_char) {
        assert!(msg.is_null());
        CALLED.store(true, Ordering::SeqCst);
    }

    #[test]
    fn test_add_dlwarning() {
        clear_dlwarning();
        add_dlwarning("/path/to/libfoo.so", "symbol not found", Some("bar"));
        CURRENT_MSG.with(|current_msg| {
            let msg = current_msg.borrow();
            assert_eq!(*msg, "libfoo.so: symbol not found \"bar\"");
        });
    }

    #[test]
    fn test_multiple_warnings() {
        clear_dlwarning();
        add_dlwarning("liba.so", "error 1", None);
        add_dlwarning("libb.so", "error 2", Some("detail"));
        CURRENT_MSG.with(|current_msg| {
            let msg = current_msg.borrow();
            assert_eq!(*msg, "liba.so: error 1\nlibb.so: error 2 \"detail\"");
        });
    }

    #[test]
    fn test_get_dlwarning_clears() {
        clear_dlwarning();
        add_dlwarning("libtest.so", "test warning", None);
        CALLED.store(false, Ordering::SeqCst);

        get_dlwarning(std::ptr::null_mut(), Some(check_warning_cb));
        assert!(CALLED.load(Ordering::SeqCst));

        CURRENT_MSG.with(|current_msg| {
            assert!(current_msg.borrow().is_empty());
        });
    }

    #[test]
    fn test_get_dlwarning_empty() {
        clear_dlwarning();
        CALLED.store(false, Ordering::SeqCst);

        get_dlwarning(std::ptr::null_mut(), Some(null_warning_cb));
        assert!(CALLED.load(Ordering::SeqCst));
    }
}

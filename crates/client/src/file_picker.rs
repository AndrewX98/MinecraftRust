use std::ffi::{c_char, CStr};
use std::process::Command;

pub(crate) struct FilePicker {
    title: String,
    file_name: String,
    mode: i32,
    patterns: Vec<String>,
    picked_file: Vec<u8>,
}

#[no_mangle]
pub extern "C" fn rust_filepicker_create() -> *mut FilePicker {
    Box::into_raw(Box::new(FilePicker {
        title: String::new(),
        file_name: String::new(),
        mode: 0,
        patterns: Vec::new(),
        picked_file: Vec::new(),
    }))
}

#[no_mangle]
pub unsafe extern "C" fn rust_filepicker_set_title(picker: *mut FilePicker, title: *const c_char) {
    if let Some(p) = picker.as_mut() {
        p.title = CStr::from_ptr(title).to_string_lossy().into_owned();
    }
}

#[no_mangle]
pub unsafe extern "C" fn rust_filepicker_set_filename(picker: *mut FilePicker, name: *const c_char) {
    if let Some(p) = picker.as_mut() {
        p.file_name = CStr::from_ptr(name).to_string_lossy().into_owned();
    }
}

#[no_mangle]
pub unsafe extern "C" fn rust_filepicker_set_mode(picker: *mut FilePicker, mode: i32) {
    if let Some(p) = picker.as_mut() {
        p.mode = mode;
    }
}

#[no_mangle]
pub unsafe extern "C" fn rust_filepicker_set_filters(
    picker: *mut FilePicker,
    patterns: *const *const c_char,
    count: i32,
) {
    if let Some(p) = picker.as_mut() {
        p.patterns.clear();
        for i in 0..count {
            let ptr = unsafe { *patterns.offset(i as isize) };
            p.patterns.push(unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned());
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn rust_filepicker_show(picker: *mut FilePicker) -> bool {
    let picker = match picker.as_mut() {
        Some(p) => p,
        None => return false,
    };
    let mut cmd = Command::new("zenity");
    cmd.arg("--file-selection");
    if !picker.title.is_empty() {
        cmd.arg("--title");
        cmd.arg(&picker.title);
    }
    if picker.mode == 1 {
        cmd.arg("--save");
        if !picker.file_name.is_empty() {
            cmd.arg("--filename");
            cmd.arg(&picker.file_name);
        }
    }
    if !picker.patterns.is_empty() {
        cmd.arg("--file-filter");
        cmd.arg(picker.patterns.join(" "));
    }
    let output = match cmd.output() {
        Ok(o) => o,
        Err(_) => return false,
    };
    if output.status.success() {
        let trimmed = output.stdout
            .strip_suffix(b"\n").or_else(|| output.stdout.strip_suffix(b"\r\n"))
            .unwrap_or(&output.stdout)
            .to_vec();
        picker.picked_file = trimmed;
        true
    } else {
        false
    }
}

#[no_mangle]
pub unsafe extern "C" fn rust_filepicker_get_picked_file(picker: *mut FilePicker) -> *const c_char {
    let picker = match picker.as_ref() {
        Some(p) => p,
        None => return std::ptr::null(),
    };
    if picker.picked_file.is_empty() {
        return std::ptr::null();
    }
    picker.picked_file.as_ptr() as *const c_char
}

#[no_mangle]
pub unsafe extern "C" fn rust_filepicker_destroy(picker: *mut FilePicker) {
    if !picker.is_null() {
        drop(Box::from_raw(picker));
    }
}

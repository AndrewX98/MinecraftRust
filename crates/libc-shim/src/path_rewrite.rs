use std::ffi::c_char;
use std::sync::OnceLock;

static REWRITE: OnceLock<Vec<(Vec<u8>, Vec<u8>)>> = OnceLock::new();

fn get_rules() -> &'static [(Vec<u8>, Vec<u8>)] {
    REWRITE.get().map_or(&[], |v| v.as_slice())
}

/// Set rewrite rules (replaces any existing).
pub fn set_rules(rules: &[(String, String)]) {
    let v: Vec<(Vec<u8>, Vec<u8>)> = rules.iter()
        .map(|(f, t)| (f.as_bytes().to_vec(), t.as_bytes().to_vec()))
        .collect();
    REWRITE.set(v).ok();
}

/// Clear all rules.
pub fn clear_rules() {
    let _ = REWRITE.set(Vec::new());
}

/// Rewrite a NUL-terminated C string path in place (up to `buf_len` bytes).
/// Returns the length of the rewritten path, or 0 if the original path fits without rewriting.
/// If the rewritten path is longer than `buf_len - 1`, nothing is written and 0 is returned.
pub unsafe fn rewrite_path_inplace(path: *mut c_char, buf_len: usize) -> usize {
    if path.is_null() {
        return 0;
    }
    let len = libc::strlen(path);
    let slice = std::slice::from_raw_parts(path as *const u8, len);
    for (from, to) in get_rules() {
        if slice.starts_with(from.as_slice()) {
            let new_len = to.len() + slice.len() - from.len();
            if new_len >= buf_len {
                return 0;
            }
            let dst = std::slice::from_raw_parts_mut(path as *mut u8, buf_len);
            // Copy replacement prefix, then the remaining suffix
            let suffix = &slice[from.len()..];
            dst[..to.len()].copy_from_slice(to);
            dst[to.len()..to.len() + suffix.len()].copy_from_slice(suffix);
            dst[new_len] = 0;
            return new_len;
        }
    }
    0
}

/// Rewrite a NUL-terminated C string path. Returns a pointer to the rewritten path.
/// If no rule matches, returns the original path pointer unchanged.
/// When a rewrite occurs, the result is a leaked allocation (valid for program lifetime).
pub unsafe fn rewrite_path(path: *const c_char) -> *const c_char {
    if path.is_null() {
        return path;
    }
    let len = libc::strlen(path);
    let slice = std::slice::from_raw_parts(path as *const u8, len);
    for (from, to) in get_rules() {
        if slice.starts_with(from.as_slice()) {
            let mut new: Vec<u8> = Vec::with_capacity(to.len() + slice.len() - from.len() + 1);
            new.extend_from_slice(to);
            new.extend_from_slice(&slice[from.len()..]);
            new.push(0);
            let ptr = new.as_ptr() as *const c_char;
            std::mem::forget(new);
            return ptr;
        }
    }
    path
}

/// C-callable path rewrite — used by C variadic wrappers (variadic.c).
/// Returns a pointer (possibly leaked) to the rewritten path, or the original.
#[no_mangle]
pub unsafe extern "C" fn shim_internal_rewrite_path(path: *const c_char) -> *const c_char {
    rewrite_path(path)
}

/// Keep a reference so linker --gc-sections doesn't strip the function.
#[used]
static _SHIM_INTERNAL_REWRITE_PATH_REF: unsafe extern "C" fn(*const c_char) -> *const c_char = shim_internal_rewrite_path;

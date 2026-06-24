use crate::types::*;
use crate::vm::JNIEnv;
use std::ffi::{CStr, CString};

pub unsafe extern "C" fn jni_NewString(_env: *mut JNIEnv, unicode: *const jchar, len: jsize) -> jstring {
    if unicode.is_null() || len <= 0 { return std::ptr::null_mut(); }
    let slice = std::slice::from_raw_parts(unicode, len as usize);
    let s = String::from_utf16_lossy(slice);
    CString::new(s).unwrap_or_default().into_raw() as jstring
}
pub unsafe extern "C" fn jni_GetStringLength(_env: *mut JNIEnv, s: jstring) -> jsize {
    if s.is_null() { return 0; }
    CStr::from_ptr(s as *const i8).to_bytes().len() as jsize
}
pub unsafe extern "C" fn jni_GetStringChars(_env: *mut JNIEnv, s: jstring, isCopy: *mut jboolean) -> *const jchar {
    if s.is_null() { return std::ptr::null(); }
    if !isCopy.is_null() { *isCopy = 1; }
    let cstr = CStr::from_ptr(s as *const i8);
    let utf16: Vec<jchar> = cstr.to_string_lossy().encode_utf16().collect();
    let ptr = utf16.as_ptr();
    std::mem::forget(utf16);
    ptr
}
pub unsafe extern "C" fn jni_ReleaseStringChars(_env: *mut JNIEnv, _s: jstring, _chars: *const jchar) {}
pub unsafe extern "C" fn jni_NewStringUTF(_env: *mut JNIEnv, utf: *const i8) -> jstring {
    if utf.is_null() { return std::ptr::null_mut(); }
    let owned = CStr::from_ptr(utf).to_string_lossy().into_owned();
    CString::new(owned).unwrap_or_default().into_raw() as jstring
}
pub unsafe extern "C" fn jni_GetStringUTFLength(_env: *mut JNIEnv, s: jstring) -> jsize {
    if s.is_null() { return 0; }
    CStr::from_ptr(s as *const i8).to_bytes().len() as jsize
}
pub unsafe extern "C" fn jni_GetStringUTFChars(_env: *mut JNIEnv, s: jstring, isCopy: *mut jboolean) -> *const i8 {
    if s.is_null() { return std::ptr::null(); }
    if !isCopy.is_null() { *isCopy = 0; }
    s as *const i8
}
pub unsafe extern "C" fn jni_ReleaseStringUTFChars(_env: *mut JNIEnv, _s: jstring, _utf: *const i8) {}
pub unsafe extern "C" fn jni_GetStringRegion(_env: *mut JNIEnv, _str: jstring, _start: jsize, _len: jsize, _buf: *mut jchar) {}
pub unsafe extern "C" fn jni_GetStringUTFRegion(_env: *mut JNIEnv, _str: jstring, _start: jsize, _len: jsize, _buf: *mut i8) {}
pub unsafe extern "C" fn jni_GetStringCritical(env: *mut JNIEnv, s: jstring, isCopy: *mut jboolean) -> *const jchar {
    jni_GetStringChars(env, s, isCopy)
}
pub unsafe extern "C" fn jni_ReleaseStringCritical(_env: *mut JNIEnv, _s: jstring, _cstring: *const jchar) {}

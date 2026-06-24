use crate::types::*;
use crate::vm::JNIEnv;
use std::ffi::CStr;

pub unsafe extern "C" fn jni_Throw(_env: *mut JNIEnv, _obj: jthrowable) -> jint { 0 }
pub unsafe extern "C" fn jni_ThrowNew(_env: *mut JNIEnv, _clazz: jclass, _msg: *const i8) -> jint { 0 }
pub unsafe extern "C" fn jni_ExceptionOccurred(_env: *mut JNIEnv) -> jthrowable { std::ptr::null_mut() }
pub unsafe extern "C" fn jni_ExceptionDescribe(_env: *mut JNIEnv) {}
pub unsafe extern "C" fn jni_ExceptionClear(_env: *mut JNIEnv) {}
pub unsafe extern "C" fn jni_FatalError(_env: *mut JNIEnv, msg: *const i8) {
    if !msg.is_null() {
        let s = CStr::from_ptr(msg).to_string_lossy();
        panic!("JNI FatalError: {}", s);
    } else {
        panic!("JNI FatalError");
    }
}
pub unsafe extern "C" fn jni_ExceptionCheck(_env: *mut JNIEnv) -> jboolean { 0 }

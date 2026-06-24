use crate::types::*;
use crate::vm::JNIEnv;
use std::ffi::c_void;

pub unsafe extern "C" fn jni_NewDirectByteBuffer(_env: *mut JNIEnv, _address: *mut c_void, _capacity: jlong) -> jobject { std::ptr::null_mut() }
pub unsafe extern "C" fn jni_GetDirectBufferAddress(_env: *mut JNIEnv, _buf: jobject) -> *mut c_void { std::ptr::null_mut() }
pub unsafe extern "C" fn jni_GetDirectBufferCapacity(_env: *mut JNIEnv, _buf: jobject) -> jlong { 0 }

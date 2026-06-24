use crate::types::*;
use crate::vm::JNIEnv;

pub unsafe extern "C" fn jni_PushLocalFrame(_env: *mut JNIEnv, _capacity: jint) -> jint { 0 }
pub unsafe extern "C" fn jni_PopLocalFrame(_env: *mut JNIEnv, result: jobject) -> jobject { result }
pub unsafe extern "C" fn jni_EnsureLocalCapacity(_env: *mut JNIEnv, _capacity: jint) -> jint { 0 }

use crate::types::*;
use crate::vm::JNIEnv;
use crate::state::jvm_state;

pub unsafe extern "C" fn jni_NewGlobalRef(_env: *mut JNIEnv, obj: jobject) -> jobject {
    if obj.is_null() { return std::ptr::null_mut(); }
    let mut state = jvm_state().lock().unwrap();
    state.globals.push(obj);
    obj
}
pub unsafe extern "C" fn jni_DeleteGlobalRef(_env: *mut JNIEnv, obj: jobject) {
    if obj.is_null() { return; }
    let mut state = jvm_state().lock().unwrap();
    state.globals.retain(|&x| x != obj);
}
pub unsafe extern "C" fn jni_DeleteLocalRef(_env: *mut JNIEnv, _localRef: jobject) {}
pub unsafe extern "C" fn jni_IsSameObject(_env: *mut JNIEnv, ref1: jobject, ref2: jobject) -> jboolean { (ref1 == ref2) as jboolean }
pub unsafe extern "C" fn jni_NewLocalRef(_env: *mut JNIEnv, obj: jobject) -> jobject { obj }
pub unsafe extern "C" fn jni_NewWeakGlobalRef(_env: *mut JNIEnv, obj: jobject) -> jweak { obj }
pub unsafe extern "C" fn jni_DeleteWeakGlobalRef(_env: *mut JNIEnv, _obj: jweak) {}
pub unsafe extern "C" fn jni_GetObjectRefType(_env: *mut JNIEnv, obj: jobject) -> jobjectRefType {
    if obj.is_null() { crate::types::JNIInvalidRefType } else { crate::types::JNIGlobalRefType }
}

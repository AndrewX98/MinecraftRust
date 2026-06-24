use crate::types::*;
use crate::vm::JNIEnv;
use crate::read_args::read_jvalue_args;

pub unsafe extern "C" fn jni_NewObject(_env: *mut JNIEnv, _clazz: jclass, _mid: jmethodID, _a1: i64, _a2: i64, _a3: i64, _a4: i64) -> jobject {
    Box::into_raw(Box::new(1u8)) as jobject
}
pub unsafe extern "C" fn jni_NewObjectV(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, args: *mut jvalue) -> jobject {
    let (a1, a2, a3, a4) = read_jvalue_args(args);
    jni_NewObject(env, clazz, mid, a1, a2, a3, a4)
}
pub unsafe extern "C" fn jni_NewObjectA(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, args: *mut jvalue) -> jobject {
    let (a1, a2, a3, a4) = read_jvalue_args(args);
    jni_NewObject(env, clazz, mid, a1, a2, a3, a4)
}
pub unsafe extern "C" fn jni_AllocObject(_env: *mut JNIEnv, _clazz: jclass) -> jobject { std::ptr::null_mut() }

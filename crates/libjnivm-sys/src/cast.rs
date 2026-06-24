use crate::types::*;
use crate::vm::JNIEnv;

pub fn tc<R>(
    f: unsafe extern "C" fn(*mut JNIEnv, jobject, jmethodID, i64, i64, i64, i64) -> R,
) -> unsafe extern "C" fn(*mut JNIEnv, jobject, jmethodID) -> R {
    unsafe { std::mem::transmute(f) }
}
pub fn tcn<R>(
    f: unsafe extern "C" fn(*mut JNIEnv, jobject, jclass, jmethodID, i64, i64, i64, i64) -> R,
) -> unsafe extern "C" fn(*mut JNIEnv, jobject, jclass, jmethodID) -> R {
    unsafe { std::mem::transmute(f) }
}
pub fn tcs<R>(
    f: unsafe extern "C" fn(*mut JNIEnv, jclass, jmethodID, i64, i64, i64, i64) -> R,
) -> unsafe extern "C" fn(*mut JNIEnv, jclass, jmethodID) -> R {
    unsafe { std::mem::transmute(f) }
}
pub fn tno(
    f: unsafe extern "C" fn(*mut JNIEnv, jclass, jmethodID, i64, i64, i64, i64) -> jobject,
) -> unsafe extern "C" fn(*mut JNIEnv, jclass, jmethodID) -> jobject {
    unsafe { std::mem::transmute(f) }
}

use crate::types::*;
use crate::vm::JNIEnv;
use std::ffi::CStr;

macro_rules! get_set_field {
    ($get:ident, $set:ident, $ty:ty, $default:expr) => {
        pub unsafe extern "C" fn $get(_env: *mut JNIEnv, _obj: jobject, _fieldID: jfieldID) -> $ty { $default }
        pub unsafe extern "C" fn $set(_env: *mut JNIEnv, _obj: jobject, _fieldID: jfieldID, _value: $ty) {}
    };
    ($get:ident, $set:ident, $ty:ty, $default:expr, static $clazz:ident) => {
        pub unsafe extern "C" fn $get(_env: *mut JNIEnv, $clazz: jclass, _fieldID: jfieldID) -> $ty { $default }
        pub unsafe extern "C" fn $set(_env: *mut JNIEnv, $clazz: jclass, _fieldID: jfieldID, _value: $ty) {}
    };
}

pub unsafe extern "C" fn jni_GetFieldID(_env: *mut JNIEnv, _clazz: jclass, name: *const i8, sig: *const i8) -> jfieldID {
    if name.is_null() || sig.is_null() { return std::ptr::null_mut(); }
    let n = CStr::from_ptr(name).to_string_lossy().into_owned();
    let s = CStr::from_ptr(sig).to_string_lossy().into_owned();
    Box::into_raw(Box::new((n, s))) as jfieldID
}

get_set_field!(jni_GetObjectField, jni_SetObjectField, jobject, std::ptr::null_mut());
get_set_field!(jni_GetBooleanField, jni_SetBooleanField, jboolean, 0);
get_set_field!(jni_GetByteField, jni_SetByteField, jbyte, 0);
get_set_field!(jni_GetCharField, jni_SetCharField, jchar, 0);
get_set_field!(jni_GetShortField, jni_SetShortField, jshort, 0);
get_set_field!(jni_GetIntField, jni_SetIntField, jint, 0);
get_set_field!(jni_GetLongField, jni_SetLongField, jlong, 0);
get_set_field!(jni_GetFloatField, jni_SetFloatField, jfloat, 0.0);
get_set_field!(jni_GetDoubleField, jni_SetDoubleField, jdouble, 0.0);

pub unsafe extern "C" fn jni_GetStaticFieldID(env: *mut JNIEnv, clazz: jclass, name: *const i8, sig: *const i8) -> jfieldID {
    jni_GetFieldID(env, clazz, name, sig)
}
get_set_field!(jni_GetStaticObjectField, jni_SetStaticObjectField, jobject, std::ptr::null_mut(), static _clazz);
get_set_field!(jni_GetStaticBooleanField, jni_SetStaticBooleanField, jboolean, 0, static _clazz);
get_set_field!(jni_GetStaticByteField, jni_SetStaticByteField, jbyte, 0, static _clazz);
get_set_field!(jni_GetStaticCharField, jni_SetStaticCharField, jchar, 0, static _clazz);
get_set_field!(jni_GetStaticShortField, jni_SetStaticShortField, jshort, 0, static _clazz);
get_set_field!(jni_GetStaticIntField, jni_SetStaticIntField, jint, 0, static _clazz);
get_set_field!(jni_GetStaticLongField, jni_SetStaticLongField, jlong, 0, static _clazz);
get_set_field!(jni_GetStaticFloatField, jni_SetStaticFloatField, jfloat, 0.0, static _clazz);
get_set_field!(jni_GetStaticDoubleField, jni_SetStaticDoubleField, jdouble, 0.0, static _clazz);

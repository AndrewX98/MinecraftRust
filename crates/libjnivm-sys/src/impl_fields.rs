use crate::types::*;
use crate::vm::JNIEnv;
use crate::state::{get_field_id_name, set_field, get_field};
use std::ffi::CStr;

macro_rules! def_get_field {
    ($name:ident, $ty:ty, $variant:ident, $default:expr) => {
        pub unsafe extern "C" fn $name(_env: *mut JNIEnv, obj: jobject, fieldID: jfieldID) -> $ty {
            if let Some((name, _sig)) = get_field_id_name(fieldID) {
                if let Some(v) = get_field(obj, &name) {
                    return v.$variant;
                }
            }
            $default
        }
    };
}

macro_rules! def_set_field {
    ($name:ident, $ty:ty, $variant:ident) => {
        pub unsafe extern "C" fn $name(_env: *mut JNIEnv, obj: jobject, fieldID: jfieldID, value: $ty) {
            if let Some((name, _sig)) = get_field_id_name(fieldID) {
                set_field(obj, &name, jvalue { $variant: value });
            }
        }
    };
}

macro_rules! def_static_get_set {
    ($get:ident, $set:ident, $ty:ty, $variant:ident, $default:expr) => {
        pub unsafe extern "C" fn $get(_env: *mut JNIEnv, _clazz: jclass, fieldID: jfieldID) -> $ty {
            if let Some((name, _sig)) = get_field_id_name(fieldID) {
                if let Some(v) = get_field(0x1 as jobject, &name) {
                    return v.$variant;
                }
            }
            $default
        }
        pub unsafe extern "C" fn $set(_env: *mut JNIEnv, _clazz: jclass, fieldID: jfieldID, value: $ty) {
            if let Some((name, _sig)) = get_field_id_name(fieldID) {
                set_field(0x1 as jobject, &name, jvalue { $variant: value });
            }
        }
    };
}

pub unsafe extern "C" fn jni_GetFieldID(_env: *mut JNIEnv, _clazz: jclass, name: *const i8, sig: *const i8) -> jfieldID {
    if name.is_null() || sig.is_null() { return std::ptr::null_mut(); }
    let n = CStr::from_ptr(name).to_string_lossy().into_owned();
    let s = CStr::from_ptr(sig).to_string_lossy().into_owned();
    Box::into_raw(Box::new((n, s))) as jfieldID
}

def_get_field!(jni_GetObjectField, jobject, l, std::ptr::null_mut());
def_set_field!(jni_SetObjectField, jobject, l);
def_get_field!(jni_GetBooleanField, jboolean, z, 0);
def_set_field!(jni_SetBooleanField, jboolean, z);
def_get_field!(jni_GetByteField, jbyte, b, 0);
def_set_field!(jni_SetByteField, jbyte, b);
def_get_field!(jni_GetCharField, jchar, c, 0);
def_set_field!(jni_SetCharField, jchar, c);
def_get_field!(jni_GetShortField, jshort, s, 0);
def_set_field!(jni_SetShortField, jshort, s);
def_get_field!(jni_GetIntField, jint, i, 0);
def_set_field!(jni_SetIntField, jint, i);
def_get_field!(jni_GetLongField, jlong, j, 0);
def_set_field!(jni_SetLongField, jlong, j);
def_get_field!(jni_GetFloatField, jfloat, f, 0.0);
def_set_field!(jni_SetFloatField, jfloat, f);
def_get_field!(jni_GetDoubleField, jdouble, d, 0.0);
def_set_field!(jni_SetDoubleField, jdouble, d);

pub unsafe extern "C" fn jni_GetStaticFieldID(env: *mut JNIEnv, clazz: jclass, name: *const i8, sig: *const i8) -> jfieldID {
    jni_GetFieldID(env, clazz, name, sig)
}
def_static_get_set!(jni_GetStaticObjectField, jni_SetStaticObjectField, jobject, l, std::ptr::null_mut());
def_static_get_set!(jni_GetStaticBooleanField, jni_SetStaticBooleanField, jboolean, z, 0);
def_static_get_set!(jni_GetStaticByteField, jni_SetStaticByteField, jbyte, b, 0);
def_static_get_set!(jni_GetStaticCharField, jni_SetStaticCharField, jchar, c, 0);
def_static_get_set!(jni_GetStaticShortField, jni_SetStaticShortField, jshort, s, 0);
def_static_get_set!(jni_GetStaticIntField, jni_SetStaticIntField, jint, i, 0);
def_static_get_set!(jni_GetStaticLongField, jni_SetStaticLongField, jlong, j, 0);
def_static_get_set!(jni_GetStaticFloatField, jni_SetStaticFloatField, jfloat, f, 0.0);
def_static_get_set!(jni_GetStaticDoubleField, jni_SetStaticDoubleField, jdouble, d, 0.0);

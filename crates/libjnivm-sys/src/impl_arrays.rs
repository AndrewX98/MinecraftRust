use crate::types::*;
use crate::vm::JNIEnv;
use std::ffi::c_void;

pub unsafe extern "C" fn jni_GetArrayLength(_env: *mut JNIEnv, array: jarray) -> jsize {
    if array.is_null() { return 0; }
    let v = &*(array as *const Vec<u8>);
    v.len() as jsize
}
pub unsafe extern "C" fn jni_NewObjectArray(_env: *mut JNIEnv, len: jsize, _clazz: jclass, _init: jobject) -> jobjectArray {
    let v: Vec<jobject> = vec![std::ptr::null_mut(); len as usize];
    Box::into_raw(Box::new(v)) as jobjectArray
}
pub unsafe extern "C" fn jni_GetObjectArrayElement(_env: *mut JNIEnv, array: jobjectArray, index: jsize) -> jobject {
    if array.is_null() { return std::ptr::null_mut(); }
    let v = &*(array as *const Vec<jobject>);
    v[index as usize]
}
pub unsafe extern "C" fn jni_SetObjectArrayElement(_env: *mut JNIEnv, array: jobjectArray, index: jsize, val: jobject) {
    if array.is_null() { return; }
    let v = &mut *(array as *mut Vec<jobject>);
    if (index as usize) < v.len() { v[index as usize] = val; }
}

macro_rules! make_array_funcs {
    ($New:ident, $GetElements:ident, $ReleaseElements:ident, $GetRegion:ident, $SetRegion:ident, $elems_ty:ty) => {
        pub unsafe extern "C" fn $New(_env: *mut JNIEnv, len: jsize) -> jobject {
            let v = vec![<$elems_ty>::default(); len as usize];
            Box::into_raw(Box::new(v)) as jobject
        }
        pub unsafe extern "C" fn $GetElements(_env: *mut JNIEnv, arr: jobject, _isCopy: *mut jboolean) -> *mut $elems_ty {
            let v = &mut *(arr as *mut Vec<$elems_ty>);
            v.as_mut_ptr()
        }
        pub unsafe extern "C" fn $ReleaseElements(_env: *mut JNIEnv, _arr: jobject, _elems: *mut $elems_ty, _mode: jint) {}
        pub unsafe extern "C" fn $GetRegion(_env: *mut JNIEnv, _arr: jobject, _start: jsize, _len: jsize, _buf: *mut $elems_ty) {}
        pub unsafe extern "C" fn $SetRegion(_env: *mut JNIEnv, _arr: jobject, _start: jsize, _len: jsize, _buf: *const $elems_ty) {}
    };
}

make_array_funcs!(jni_NewBooleanArray, jni_GetBooleanArrayElements, jni_ReleaseBooleanArrayElements, jni_GetBooleanArrayRegion, jni_SetBooleanArrayRegion, jboolean);
make_array_funcs!(jni_NewByteArray, jni_GetByteArrayElements, jni_ReleaseByteArrayElements, jni_GetByteArrayRegion, jni_SetByteArrayRegion, jbyte);
make_array_funcs!(jni_NewCharArray, jni_GetCharArrayElements, jni_ReleaseCharArrayElements, jni_GetCharArrayRegion, jni_SetCharArrayRegion, jchar);
make_array_funcs!(jni_NewShortArray, jni_GetShortArrayElements, jni_ReleaseShortArrayElements, jni_GetShortArrayRegion, jni_SetShortArrayRegion, jshort);
make_array_funcs!(jni_NewIntArray, jni_GetIntArrayElements, jni_ReleaseIntArrayElements, jni_GetIntArrayRegion, jni_SetIntArrayRegion, jint);
make_array_funcs!(jni_NewLongArray, jni_GetLongArrayElements, jni_ReleaseLongArrayElements, jni_GetLongArrayRegion, jni_SetLongArrayRegion, jlong);
make_array_funcs!(jni_NewFloatArray, jni_GetFloatArrayElements, jni_ReleaseFloatArrayElements, jni_GetFloatArrayRegion, jni_SetFloatArrayRegion, jfloat);
make_array_funcs!(jni_NewDoubleArray, jni_GetDoubleArrayElements, jni_ReleaseDoubleArrayElements, jni_GetDoubleArrayRegion, jni_SetDoubleArrayRegion, jdouble);

pub unsafe extern "C" fn jni_GetPrimitiveArrayCritical(_env: *mut JNIEnv, arr: jarray, _isCopy: *mut jboolean) -> *mut c_void {
    if arr.is_null() { return std::ptr::null_mut(); }
    let v = &mut *(arr as *mut Vec<u8>);
    v.as_mut_ptr() as *mut c_void
}
pub unsafe extern "C" fn jni_ReleasePrimitiveArrayCritical(_env: *mut JNIEnv, _arr: jarray, _carray: *mut c_void, _mode: jint) {}

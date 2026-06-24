use std::ffi::c_char;

pub type jboolean = u8;
pub type jbyte = i8;
pub type jchar = u16;
pub type jshort = i16;
pub type jint = i32;
pub type jlong = i64;
pub type jfloat = f32;
pub type jdouble = f64;
pub type jsize = i32;

pub type jobject = *mut std::ffi::c_void;
pub type jclass = jobject;
pub type jstring = jobject;
pub type jarray = jobject;
pub type jobjectArray = jobject;
pub type jbooleanArray = jobject;
pub type jbyteArray = jobject;
pub type jcharArray = jobject;
pub type jshortArray = jobject;
pub type jintArray = jobject;
pub type jlongArray = jobject;
pub type jfloatArray = jobject;
pub type jdoubleArray = jobject;
pub type jthrowable = jobject;
pub type jweak = jobject;

pub type jmethodID = *mut std::ffi::c_void;
pub type jfieldID = *mut std::ffi::c_void;

#[repr(C)]
#[derive(Clone, Copy)]
pub union jvalue {
    pub z: jboolean,
    pub b: jbyte,
    pub c: jchar,
    pub s: jshort,
    pub i: jint,
    pub j: jlong,
    pub f: jfloat,
    pub d: jdouble,
    pub l: jobject,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct JNINativeMethod {
    pub name: *const c_char,
    pub signature: *const c_char,
    pub fnPtr: *mut std::ffi::c_void,
}

pub type jobjectRefType = u32;
pub const JNIInvalidRefType: jobjectRefType = 0;
pub const JNILocalRefType: jobjectRefType = 1;
pub const JNIGlobalRefType: jobjectRefType = 2;
pub const JNIWeakGlobalRefType: jobjectRefType = 3;

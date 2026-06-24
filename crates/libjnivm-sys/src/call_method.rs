use crate::types::*;
use crate::vm::JNIEnv;
use crate::read_args::read_jvalue_args;
use crate::state::find_method;

macro_rules! def_call_base {
    ($name:ident, $ret:ty, $default:expr) => {
        pub unsafe extern "C" fn $name(env: *mut JNIEnv, obj: jobject, mid: jmethodID, a1: i64, a2: i64, a3: i64, a4: i64) -> $ret {
            if let Some(f) = find_method(mid) {
                let f: unsafe extern "C" fn(*mut JNIEnv, jobject, i64, i64, i64, i64) -> $ret = std::mem::transmute(f);
                return f(env, obj, a1, a2, a3, a4);
            }
            $default
        }
    };
}

macro_rules! def_call_variants {
    ($base:ident, $v:ident, $a:ident, $ret:ty) => {
        pub unsafe extern "C" fn $v(env: *mut JNIEnv, obj: jobject, mid: jmethodID, args: *mut jvalue) -> $ret {
            let (a1, a2, a3, a4) = read_jvalue_args(args);
            $base(env, obj, mid, a1, a2, a3, a4)
        }
        pub unsafe extern "C" fn $a(env: *mut JNIEnv, obj: jobject, mid: jmethodID, args: *mut jvalue) -> $ret {
            let (a1, a2, a3, a4) = read_jvalue_args(args);
            $base(env, obj, mid, a1, a2, a3, a4)
        }
    };
}

def_call_base!(jni_CallObjectMethod, jobject, std::ptr::null_mut());
def_call_variants!(jni_CallObjectMethod, jni_CallObjectMethodV, jni_CallObjectMethodA, jobject);
def_call_base!(jni_CallBooleanMethod, jboolean, 0);
def_call_variants!(jni_CallBooleanMethod, jni_CallBooleanMethodV, jni_CallBooleanMethodA, jboolean);
def_call_base!(jni_CallByteMethod, jbyte, 0);
def_call_variants!(jni_CallByteMethod, jni_CallByteMethodV, jni_CallByteMethodA, jbyte);
def_call_base!(jni_CallCharMethod, jchar, 0);
def_call_variants!(jni_CallCharMethod, jni_CallCharMethodV, jni_CallCharMethodA, jchar);
def_call_base!(jni_CallShortMethod, jshort, 0);
def_call_variants!(jni_CallShortMethod, jni_CallShortMethodV, jni_CallShortMethodA, jshort);
def_call_base!(jni_CallIntMethod, jint, 0);
def_call_variants!(jni_CallIntMethod, jni_CallIntMethodV, jni_CallIntMethodA, jint);
def_call_base!(jni_CallLongMethod, jlong, 0);
def_call_variants!(jni_CallLongMethod, jni_CallLongMethodV, jni_CallLongMethodA, jlong);
def_call_base!(jni_CallFloatMethod, jfloat, 0.0);
def_call_variants!(jni_CallFloatMethod, jni_CallFloatMethodV, jni_CallFloatMethodA, jfloat);
def_call_base!(jni_CallDoubleMethod, jdouble, 0.0);
def_call_variants!(jni_CallDoubleMethod, jni_CallDoubleMethodV, jni_CallDoubleMethodA, jdouble);

pub unsafe extern "C" fn jni_CallVoidMethod(env: *mut JNIEnv, obj: jobject, mid: jmethodID, a1: i64, a2: i64, a3: i64, a4: i64) {
    if let Some(f) = find_method(mid) {
        let f: unsafe extern "C" fn(*mut JNIEnv, jobject, i64, i64, i64, i64) = std::mem::transmute(f);
        f(env, obj, a1, a2, a3, a4);
    }
}
pub unsafe extern "C" fn jni_CallVoidMethodV(env: *mut JNIEnv, obj: jobject, mid: jmethodID, args: *mut jvalue) {
    let (a1, a2, a3, a4) = read_jvalue_args(args);
    jni_CallVoidMethod(env, obj, mid, a1, a2, a3, a4)
}
pub unsafe extern "C" fn jni_CallVoidMethodA(env: *mut JNIEnv, obj: jobject, mid: jmethodID, args: *mut jvalue) {
    let (a1, a2, a3, a4) = read_jvalue_args(args);
    jni_CallVoidMethod(env, obj, mid, a1, a2, a3, a4)
}

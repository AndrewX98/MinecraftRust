use crate::types::*;
use crate::vm::JNIEnv;
use crate::read_args::read_jvalue_args;
use crate::state::find_method;

macro_rules! def_static_call_base {
    ($name:ident, $ret:ty, $default:expr) => {
        pub unsafe extern "C" fn $name(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, a1: i64, a2: i64, a3: i64, a4: i64) -> $ret {
            if let Some(f) = find_method(mid) {
                let f: unsafe extern "C" fn(*mut JNIEnv, jclass, i64, i64, i64, i64) -> $ret = std::mem::transmute(f);
                return f(env, clazz, a1, a2, a3, a4);
            }
            $default
        }
    };
}

macro_rules! def_static_call_variants {
    ($base:ident, $v:ident, $a:ident, $ret:ty) => {
        pub unsafe extern "C" fn $v(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, args: *mut jvalue) -> $ret {
            let (a1, a2, a3, a4) = read_jvalue_args(args);
            $base(env, clazz, mid, a1, a2, a3, a4)
        }
        pub unsafe extern "C" fn $a(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, args: *mut jvalue) -> $ret {
            let (a1, a2, a3, a4) = read_jvalue_args(args);
            $base(env, clazz, mid, a1, a2, a3, a4)
        }
    };
}

def_static_call_base!(jni_CallStaticObjectMethod, jobject, std::ptr::null_mut());
def_static_call_variants!(jni_CallStaticObjectMethod, jni_CallStaticObjectMethodV, jni_CallStaticObjectMethodA, jobject);
def_static_call_base!(jni_CallStaticBooleanMethod, jboolean, 0);
def_static_call_variants!(jni_CallStaticBooleanMethod, jni_CallStaticBooleanMethodV, jni_CallStaticBooleanMethodA, jboolean);
def_static_call_base!(jni_CallStaticByteMethod, jbyte, 0);
def_static_call_variants!(jni_CallStaticByteMethod, jni_CallStaticByteMethodV, jni_CallStaticByteMethodA, jbyte);
def_static_call_base!(jni_CallStaticCharMethod, jchar, 0);
def_static_call_variants!(jni_CallStaticCharMethod, jni_CallStaticCharMethodV, jni_CallStaticCharMethodA, jchar);
def_static_call_base!(jni_CallStaticShortMethod, jshort, 0);
def_static_call_variants!(jni_CallStaticShortMethod, jni_CallStaticShortMethodV, jni_CallStaticShortMethodA, jshort);
def_static_call_base!(jni_CallStaticIntMethod, jint, 0);
def_static_call_variants!(jni_CallStaticIntMethod, jni_CallStaticIntMethodV, jni_CallStaticIntMethodA, jint);
def_static_call_base!(jni_CallStaticLongMethod, jlong, 0);
def_static_call_variants!(jni_CallStaticLongMethod, jni_CallStaticLongMethodV, jni_CallStaticLongMethodA, jlong);
def_static_call_base!(jni_CallStaticFloatMethod, jfloat, 0.0);
def_static_call_variants!(jni_CallStaticFloatMethod, jni_CallStaticFloatMethodV, jni_CallStaticFloatMethodA, jfloat);
def_static_call_base!(jni_CallStaticDoubleMethod, jdouble, 0.0);
def_static_call_variants!(jni_CallStaticDoubleMethod, jni_CallStaticDoubleMethodV, jni_CallStaticDoubleMethodA, jdouble);

pub unsafe extern "C" fn jni_CallStaticVoidMethod(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, a1: i64, a2: i64, a3: i64, a4: i64) {
    if let Some(f) = find_method(mid) {
        let f: unsafe extern "C" fn(*mut JNIEnv, jclass, i64, i64, i64, i64) = std::mem::transmute(f);
        f(env, clazz, a1, a2, a3, a4);
    }
}
pub unsafe extern "C" fn jni_CallStaticVoidMethodV(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, args: *mut jvalue) {
    let (a1, a2, a3, a4) = read_jvalue_args(args);
    jni_CallStaticVoidMethod(env, clazz, mid, a1, a2, a3, a4)
}
pub unsafe extern "C" fn jni_CallStaticVoidMethodA(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, args: *mut jvalue) {
    let (a1, a2, a3, a4) = read_jvalue_args(args);
    jni_CallStaticVoidMethod(env, clazz, mid, a1, a2, a3, a4)
}

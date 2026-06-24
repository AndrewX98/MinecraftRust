use crate::types::*;
use crate::vm::JNIEnv;
use crate::call_method::*;

macro_rules! def_nonvirtual {
    ($name:ident, $base:ident, $v:ident, $base_v:ident, $a:ident, $base_a:ident, $ret:ty) => {
        pub unsafe extern "C" fn $name(env: *mut JNIEnv, obj: jobject, _clazz: jclass, mid: jmethodID, a1: i64, a2: i64, a3: i64, a4: i64) -> $ret {
            $base(env, obj, mid, a1, a2, a3, a4)
        }
        pub unsafe extern "C" fn $v(env: *mut JNIEnv, obj: jobject, _clazz: jclass, mid: jmethodID, args: *mut jvalue) -> $ret {
            $base_v(env, obj, mid, args)
        }
        pub unsafe extern "C" fn $a(env: *mut JNIEnv, obj: jobject, _clazz: jclass, mid: jmethodID, args: *mut jvalue) -> $ret {
            $base_a(env, obj, mid, args)
        }
    };
}

def_nonvirtual!(jni_CallNonvirtualObjectMethod, jni_CallObjectMethod, jni_CallNonvirtualObjectMethodV, jni_CallObjectMethodV, jni_CallNonvirtualObjectMethodA, jni_CallObjectMethodA, jobject);
def_nonvirtual!(jni_CallNonvirtualBooleanMethod, jni_CallBooleanMethod, jni_CallNonvirtualBooleanMethodV, jni_CallBooleanMethodV, jni_CallNonvirtualBooleanMethodA, jni_CallBooleanMethodA, jboolean);
def_nonvirtual!(jni_CallNonvirtualByteMethod, jni_CallByteMethod, jni_CallNonvirtualByteMethodV, jni_CallByteMethodV, jni_CallNonvirtualByteMethodA, jni_CallByteMethodA, jbyte);
def_nonvirtual!(jni_CallNonvirtualCharMethod, jni_CallCharMethod, jni_CallNonvirtualCharMethodV, jni_CallCharMethodV, jni_CallNonvirtualCharMethodA, jni_CallCharMethodA, jchar);
def_nonvirtual!(jni_CallNonvirtualShortMethod, jni_CallShortMethod, jni_CallNonvirtualShortMethodV, jni_CallShortMethodV, jni_CallNonvirtualShortMethodA, jni_CallShortMethodA, jshort);
def_nonvirtual!(jni_CallNonvirtualIntMethod, jni_CallIntMethod, jni_CallNonvirtualIntMethodV, jni_CallIntMethodV, jni_CallNonvirtualIntMethodA, jni_CallIntMethodA, jint);
def_nonvirtual!(jni_CallNonvirtualLongMethod, jni_CallLongMethod, jni_CallNonvirtualLongMethodV, jni_CallLongMethodV, jni_CallNonvirtualLongMethodA, jni_CallLongMethodA, jlong);
def_nonvirtual!(jni_CallNonvirtualFloatMethod, jni_CallFloatMethod, jni_CallNonvirtualFloatMethodV, jni_CallFloatMethodV, jni_CallNonvirtualFloatMethodA, jni_CallFloatMethodA, jfloat);
def_nonvirtual!(jni_CallNonvirtualDoubleMethod, jni_CallDoubleMethod, jni_CallNonvirtualDoubleMethodV, jni_CallDoubleMethodV, jni_CallNonvirtualDoubleMethodA, jni_CallDoubleMethodA, jdouble);

pub unsafe extern "C" fn jni_CallNonvirtualVoidMethod(env: *mut JNIEnv, obj: jobject, _clazz: jclass, mid: jmethodID, a1: i64, a2: i64, a3: i64, a4: i64) {
    jni_CallVoidMethod(env, obj, mid, a1, a2, a3, a4)
}
pub unsafe extern "C" fn jni_CallNonvirtualVoidMethodV(env: *mut JNIEnv, obj: jobject, _clazz: jclass, mid: jmethodID, args: *mut jvalue) {
    jni_CallVoidMethodV(env, obj, mid, args)
}
pub unsafe extern "C" fn jni_CallNonvirtualVoidMethodA(env: *mut JNIEnv, obj: jobject, _clazz: jclass, mid: jmethodID, args: *mut jvalue) {
    jni_CallVoidMethodA(env, obj, mid, args)
}

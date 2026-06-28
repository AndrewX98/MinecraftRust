use crate::types::*;
use crate::vm::JNIEnv;
use crate::read_args::read_jvalue_args;
use crate::state::{jvm_state, get_class_name_from_handle, alloc_object_fields};
use std::ffi::CStr;

pub unsafe extern "C" fn jni_NewObject(_env: *mut JNIEnv, _clazz: jclass, _mid: jmethodID, _a1: i64, _a2: i64, _a3: i64, _a4: i64) -> jobject {
    let obj = Box::into_raw(Box::new(1u8)) as jobject;
    alloc_object_fields(obj);
    obj
}

unsafe fn invoke_init(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, obj: jobject, args: *mut jvalue) {
    if clazz.is_null() || mid.is_null() || obj.is_null() { return; }
    if let Some(cls_name) = get_class_name_from_handle(clazz) {
        let state = jvm_state().lock().unwrap();
        if let Some(cls) = state.classes.get(&cls_name) {
            if let Some(&f) = cls.methods.get(&("<init>".to_string(), "()V".to_string())) {
                drop(state);
                let f: unsafe extern "C" fn(*mut JNIEnv, jobject) = std::mem::transmute(f);
                f(env, obj);
                return;
            }
            if let Some(f) = cls.methods.iter().find(|((n, _s), _)| n == "<init>").map(|(_, &f)| f) {
                drop(state);
                let (a1, a2, a3, a4) = read_jvalue_args(args);
                type InitFn = unsafe extern "C" fn(*mut JNIEnv, jobject, i64, i64, i64, i64);
                let f: InitFn = std::mem::transmute(f);
                f(env, obj, a1, a2, a3, a4);
            }
        }
    }
}

pub unsafe extern "C" fn jni_NewObjectV(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, args: *mut jvalue) -> jobject {
    let obj = jni_NewObject(env, clazz, mid, 0, 0, 0, 0);
    invoke_init(env, clazz, mid, obj, args);
    obj
}
pub unsafe extern "C" fn jni_NewObjectA(env: *mut JNIEnv, clazz: jclass, mid: jmethodID, args: *mut jvalue) -> jobject {
    let obj = jni_NewObject(env, clazz, mid, 0, 0, 0, 0);
    invoke_init(env, clazz, mid, obj, args);
    obj
}
pub unsafe extern "C" fn jni_AllocObject(_env: *mut JNIEnv, clazz: jclass) -> jobject {
    let obj = Box::into_raw(Box::new(1u8)) as jobject;
    alloc_object_fields(obj);
    obj
}

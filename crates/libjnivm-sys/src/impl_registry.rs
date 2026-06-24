use crate::types::*;
use crate::vm::{JNIEnv, JavaVM};
use crate::state::{jvm_state, get_class_name_from_handle};
use std::ffi::CStr;

pub unsafe extern "C" fn jni_RegisterNatives(_env: *mut JNIEnv, clazz: jclass, methods: *const JNINativeMethod, nMethods: jint) -> jint {
    if clazz.is_null() || methods.is_null() { return -1; }
    let cls_name = match get_class_name_from_handle(clazz) {
        Some(n) => n,
        None => return -1,
    };
    let mut state = jvm_state().lock().unwrap();
    let cls = match state.classes.get_mut(&cls_name) {
        Some(c) => c,
        None => return -1,
    };
    for i in 0..nMethods as isize {
        let m = &*methods.offset(i);
        if m.name.is_null() || m.signature.is_null() || m.fnPtr.is_null() { continue; }
        let n = CStr::from_ptr(m.name).to_string_lossy().into_owned();
        let s = CStr::from_ptr(m.signature).to_string_lossy().into_owned();
        cls.methods.insert((n, s), m.fnPtr);
    }
    0
}
pub unsafe extern "C" fn jni_UnregisterNatives(_env: *mut JNIEnv, _clazz: jclass) -> jint { 0 }
pub unsafe extern "C" fn jni_MonitorEnter(_env: *mut JNIEnv, _obj: jobject) -> jint { 0 }
pub unsafe extern "C" fn jni_MonitorExit(_env: *mut JNIEnv, _obj: jobject) -> jint { 0 }
pub unsafe extern "C" fn jni_GetJavaVM(_env: *mut JNIEnv, vm: *mut JavaVM) -> jint {
    if vm.is_null() { return -1; }
    let state = jvm_state().lock().unwrap();
    *vm = state.vm_handle;
    0
}

use crate::types::*;
use crate::vm::JNIEnv;
use crate::state::{jvm_state, JniClass};
use std::collections::HashMap;
use std::ffi::CStr;

pub unsafe extern "C" fn jni_GetVersion(_env: *mut JNIEnv) -> jint { 0x10006 }
pub unsafe extern "C" fn jni_DefineClass(_env: *mut JNIEnv, _name: *const i8, _loader: jobject, _buf: *const i8, _len: jsize) -> jclass {
    std::ptr::null_mut()
}
pub unsafe extern "C" fn jni_FindClass(_env: *mut JNIEnv, name: *const i8) -> jclass {
    if name.is_null() { return std::ptr::null_mut(); }
    let s = CStr::from_ptr(name).to_string_lossy().into_owned();
    let mut state = jvm_state().lock().unwrap();
    if let Some(id) = state.handles.iter().find(|(_, v)| *v == &s).map(|(k, _)| *k) {
        return id as jclass;
    }
    let id = state.next_class_id;
    state.next_class_id += 1;
    state.handles.insert(id, s.clone());
    state.classes.insert(s.clone(), JniClass { name: s, methods: HashMap::new() });
    id as jclass
}
pub unsafe extern "C" fn jni_FromReflectedMethod(_env: *mut JNIEnv, _method: jobject) -> jmethodID { std::ptr::null_mut() }
pub unsafe extern "C" fn jni_FromReflectedField(_env: *mut JNIEnv, _field: jobject) -> jfieldID { std::ptr::null_mut() }
pub unsafe extern "C" fn jni_ToReflectedMethod(_env: *mut JNIEnv, _cls: jclass, _methodID: jmethodID, _isStatic: jboolean) -> jobject { std::ptr::null_mut() }
pub unsafe extern "C" fn jni_GetSuperclass(_env: *mut JNIEnv, _sub: jclass) -> jclass { std::ptr::null_mut() }
pub unsafe extern "C" fn jni_IsAssignableFrom(_env: *mut JNIEnv, _sub: jclass, _sup: jclass) -> jboolean { 1 }
pub unsafe extern "C" fn jni_ToReflectedField(_env: *mut JNIEnv, _cls: jclass, _fieldID: jfieldID, _isStatic: jboolean) -> jobject { std::ptr::null_mut() }

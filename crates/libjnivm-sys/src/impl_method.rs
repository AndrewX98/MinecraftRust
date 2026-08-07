use crate::types::*;
use crate::vm::JNIEnv;
use crate::state::{jvm_state, get_class_name_from_handle};
use std::ffi::CStr;

pub unsafe extern "C" fn jni_GetMethodID(_env: *mut JNIEnv, clazz: jclass, name: *const i8, sig: *const i8) -> jmethodID {
    if name.is_null() || sig.is_null() { return std::ptr::null_mut(); }
    let n = CStr::from_ptr(name).to_string_lossy().into_owned();
    let s = CStr::from_ptr(sig).to_string_lossy().into_owned();
    let cls_name = get_class_name_from_handle(clazz).unwrap_or_else(|| "<unknown>".to_string());
    let state = jvm_state().lock().unwrap();
    for (_, cls) in &state.classes {
        if let Some(&f) = cls.methods.get(&(n.clone(), s.clone())) {
            return f as jmethodID;
        }
    }
    drop(state);
    log::warn!("GetMethodID: no native registered for {}.{}{} — calls will return 0/null", cls_name, n, s);
    let tok = Box::into_raw(Box::new((n.clone(), s.clone()))) as jmethodID;
    jvm_state().lock().unwrap().method_tokens.insert(tok, (n, s));
    tok
}

pub unsafe extern "C" fn jni_GetStaticMethodID(env: *mut JNIEnv, clazz: jclass, name: *const i8, sig: *const i8) -> jmethodID {
    jni_GetMethodID(env, clazz, name, sig)
}

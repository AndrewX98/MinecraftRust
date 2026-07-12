use crate::types::*;
use crate::vm::JNIEnv;
use crate::state::jvm_state;
use std::ffi::CStr;

pub unsafe extern "C" fn jni_GetMethodID(_env: *mut JNIEnv, _clazz: jclass, name: *const i8, sig: *const i8) -> jmethodID {
    if name.is_null() || sig.is_null() { return std::ptr::null_mut(); }
    let n = CStr::from_ptr(name).to_string_lossy().into_owned();
    let s = CStr::from_ptr(sig).to_string_lossy().into_owned();
    let state = jvm_state().lock().unwrap();
    for (_, cls) in &state.classes {
        if let Some(&f) = cls.methods.get(&(n.clone(), s.clone())) {
            return f as jmethodID;
        }
    }
    drop(state);
    log::warn!("GetMethodID: no native registered for {}{} — calls will return 0/null", n, s);
    Box::into_raw(Box::new((n, s))) as jmethodID
}

pub unsafe extern "C" fn jni_GetStaticMethodID(env: *mut JNIEnv, clazz: jclass, name: *const i8, sig: *const i8) -> jmethodID {
    jni_GetMethodID(env, clazz, name, sig)
}

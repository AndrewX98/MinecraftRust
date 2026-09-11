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
    // Cross-class (name, sig) search — call sites depend on it because some
    // objects predate class tracking (null-class creations). Several classes
    // legitimately register the same signature, so the winner MUST NOT depend
    // on HashMap iteration order (it would change every boot). Exact-class
    // match wins; otherwise the lexicographically smallest class name, so the
    // choice is identical on every boot.
    let mut best: Option<(String, usize)> = None;
    for (cls_key, cls) in &state.classes {
        if cls.methods.contains_key(&(n.clone(), s.clone())) {
            let rank = if *cls_key == cls_name { 0 } else { 1 };
            let better = match &best {
                None => true,
                Some((best_name, best_rank)) => {
                    rank < *best_rank || (rank == *best_rank && *cls_key < *best_name)
                }
            };
            if better {
                best = Some((cls_key.clone(), rank));
                if rank == 0 {
                    break;
                }
            }
        }
    }
    if let Some((cls_key, _)) = best {
        if let Some(&f) = state.classes[&cls_key].methods.get(&(n.clone(), s.clone())) {
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

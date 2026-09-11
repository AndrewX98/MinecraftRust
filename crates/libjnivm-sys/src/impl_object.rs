use crate::types::*;
use crate::vm::JNIEnv;
use crate::state::{jvm_state, JniClass};
use std::collections::HashMap;

pub unsafe extern "C" fn jni_GetObjectClass(_env: *mut JNIEnv, obj: jobject) -> jclass {
    // Report the tracked creation class when known; fall back to
    // java/lang/Object for untracked handles (e.g. null-class creations).
    let name = crate::state::get_object_class(obj).unwrap_or_else(|| "java/lang/Object".to_string());
    let mut state = jvm_state().lock().unwrap();
    if let Some(id) = state.handles.iter().find(|(_, v)| *v == &name).map(|(k, _)| *k) {
        return id as jclass;
    }
    let id = state.next_class_id;
    state.next_class_id += 1;
    let name_for_map = name.clone();
    state.handles.insert(id, name);
    // Never clobber a registered class: only ensure *some* entry exists for
    // genuinely unknown names (the old code path for java/lang/Object).
    state.classes.entry(name_for_map.clone()).or_insert_with(|| JniClass { name: name_for_map, methods: HashMap::new() });
    id as jclass
}
pub unsafe extern "C" fn jni_IsInstanceOf(_env: *mut JNIEnv, _obj: jobject, _clazz: jclass) -> jboolean { 1 }

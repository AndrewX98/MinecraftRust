use crate::types::*;
use crate::vm::JNIEnv;
use crate::state::{jvm_state, JniClass};
use std::collections::HashMap;

pub unsafe extern "C" fn jni_GetObjectClass(_env: *mut JNIEnv, _obj: jobject) -> jclass {
    let name = "java/lang/Object".to_string();
    let mut state = jvm_state().lock().unwrap();
    if let Some(id) = state.handles.iter().find(|(_, v)| *v == &name).map(|(k, _)| *k) {
        return id as jclass;
    }
    let id = state.next_class_id;
    state.next_class_id += 1;
    let name_for_map = name.clone();
    state.handles.insert(id, name);
    state.classes.insert(name_for_map, JniClass { name: "java/lang/Object".to_string(), methods: HashMap::new() });
    id as jclass
}
pub unsafe extern "C" fn jni_IsInstanceOf(_env: *mut JNIEnv, _obj: jobject, _clazz: jclass) -> jboolean { 1 }

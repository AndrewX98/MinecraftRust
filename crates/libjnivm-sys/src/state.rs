use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::types::*;
use crate::vm::*;
use crate::jni_interface::JNINativeInterface;
use crate::vm_funcs;
use crate::build_native;

pub struct JniClass {
    pub name: String,
    pub methods: HashMap<(String, String), *mut std::ffi::c_void>,
}

pub struct JvmInner {
    pub classes: HashMap<String, JniClass>,
    pub globals: Vec<jobject>,
    pub vm_handle: *mut JavaVMAttrs,
    pub env_handle: *mut JNIEnvAttrs,
    pub handles: HashMap<usize, String>,
    pub next_class_id: usize,
    pub object_fields: HashMap<usize, HashMap<String, jvalue>>,
}

unsafe impl Send for JvmInner {}
unsafe impl Sync for JvmInner {}

static JVM_STATE: OnceLock<Mutex<JvmInner>> = OnceLock::new();

pub fn jvm_state() -> &'static Mutex<JvmInner> {
    JVM_STATE.get_or_init(|| {
        let vm_iface = Box::into_raw(Box::new(vm_funcs::build_vm_interface()));
        let vm = Box::into_raw(Box::new(JavaVMAttrs { functions: vm_iface }));
        let iface = Box::into_raw(Box::new(build_native::build_native_interface()));
        let env = Box::into_raw(Box::new(JNIEnvAttrs { functions: iface }));
        Mutex::new(JvmInner {
            classes: HashMap::new(),
            globals: Vec::new(),
            vm_handle: vm,
            env_handle: env,
            handles: HashMap::new(),
            next_class_id: 1,
            object_fields: HashMap::new(),
        })
    })
}

pub fn get_jnienv_from_env(env: *mut JNIEnv) -> *mut JNIEnvAttrs {
    env as *mut JNIEnvAttrs
}

pub fn get_iface_from_env(env: *mut JNIEnv) -> *mut JNINativeInterface {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        let attrs = env as *mut JNIEnvAttrs;
        (*attrs).functions
    }
}

pub fn find_method(mid: jmethodID) -> Option<*mut std::ffi::c_void> {
    if mid.is_null() { return None; }
    let state = jvm_state().lock().unwrap();
    for (_, cls) in &state.classes {
        for ((_, _), &f) in &cls.methods {
            if f as jmethodID == mid {
                return Some(f);
            }
        }
    }
    None
}

pub fn get_class_name_from_handle(clazz: jclass) -> Option<String> {
    if clazz.is_null() { return None; }
    let id = clazz as usize;
    let state = jvm_state().lock().unwrap();
    state.handles.get(&id).cloned()
}

pub fn get_field_id_name(fieldID: jfieldID) -> Option<(String, String)> {
    if fieldID.is_null() { return None; }
    unsafe {
        let pair = fieldID as *const (String, String);
        Some(((*pair).0.clone(), (*pair).1.clone()))
    }
}

pub fn set_field(obj: jobject, field_name: &str, value: jvalue) {
    if obj.is_null() { return; }
    let mut state = jvm_state().lock().unwrap();
    state.object_fields
        .entry(obj as usize)
        .or_insert_with(HashMap::new)
        .insert(field_name.to_string(), value);
}

pub fn get_field(obj: jobject, field_name: &str) -> Option<jvalue> {
    if obj.is_null() { return None; }
    let state = jvm_state().lock().unwrap();
    state.object_fields
        .get(&(obj as usize))
        .and_then(|fields| fields.get(field_name))
        .copied()
}

pub fn alloc_object_fields(obj: jobject) {
    if obj.is_null() { return; }
    let mut state = jvm_state().lock().unwrap();
    state.object_fields.entry(obj as usize).or_insert_with(HashMap::new);
}

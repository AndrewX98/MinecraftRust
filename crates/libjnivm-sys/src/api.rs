use crate::types::*;
use crate::vm::{JavaVM, JNIEnv};
use crate::state::{jvm_state, get_iface_from_env};

#[no_mangle]
pub unsafe extern "C" fn jnivm_create_vm() -> *mut JavaVM {
    let state = jvm_state().lock().unwrap();
    state.vm_handle as *mut JavaVM
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_destroy_vm(_vm: *mut JavaVM) {}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_env(_vm: *mut JavaVM) -> *mut JNIEnv {
    let state = jvm_state().lock().unwrap();
    state.env_handle as *mut JNIEnv
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_find_class(env: *mut JNIEnv, name: *const i8) -> jclass {
    let iface = get_iface_from_env(env);
    if iface.is_null() { return std::ptr::null_mut(); }
    if (*iface).FindClass.is_none() { return std::ptr::null_mut(); }
    (*iface).FindClass.unwrap()(env, name)
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_method_id(env: *mut JNIEnv, clazz: jclass, name: *const i8, sig: *const i8) -> jmethodID {
    let iface = get_iface_from_env(env);
    (*iface).GetMethodID.unwrap()(env, clazz, name, sig)
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_static_method_id(env: *mut JNIEnv, clazz: jclass, name: *const i8, sig: *const i8) -> jmethodID {
    let iface = get_iface_from_env(env);
    (*iface).GetStaticMethodID.unwrap()(env, clazz, name, sig)
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_field_id(env: *mut JNIEnv, clazz: jclass, name: *const i8, sig: *const i8) -> jfieldID {
    let iface = get_iface_from_env(env);
    (*iface).GetFieldID.unwrap()(env, clazz, name, sig)
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_static_field_id(env: *mut JNIEnv, clazz: jclass, name: *const i8, sig: *const i8) -> jfieldID {
    let iface = get_iface_from_env(env);
    (*iface).GetStaticFieldID.unwrap()(env, clazz, name, sig)
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_register_natives(env: *mut JNIEnv, clazz: jclass, methods: *const JNINativeMethod, count: jint) -> jint {
    let iface = get_iface_from_env(env);
    (*iface).RegisterNatives.unwrap()(env, clazz, methods, count)
}

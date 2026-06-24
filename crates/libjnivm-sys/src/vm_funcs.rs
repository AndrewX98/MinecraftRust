use crate::types::*;
use crate::vm::*;

pub fn build_vm_interface() -> JavaVMInterface {
    JavaVMInterface {
        reserved0: None,
        reserved1: None,
        reserved2: None,
        DestroyJavaVM: Some(jni_DestroyJavaVM),
        AttachCurrentThread: Some(jni_AttachCurrentThread),
        DetachCurrentThread: Some(jni_DetachCurrentThread),
        GetEnv: Some(jni_GetEnv),
        AttachCurrentThreadAsDaemon: Some(jni_AttachCurrentThreadAsDaemon),
    }
}

pub unsafe extern "C" fn jni_DestroyJavaVM(_vm: *mut JavaVM) -> jint { 0 }
pub unsafe extern "C" fn jni_AttachCurrentThread(
    _vm: *mut JavaVM, _penv: *mut *mut std::ffi::c_void, _args: *mut std::ffi::c_void,
) -> jint {
    0
}
pub unsafe extern "C" fn jni_DetachCurrentThread(_vm: *mut JavaVM) -> jint { 0 }
pub unsafe extern "C" fn jni_GetEnv(
    _vm: *mut JavaVM, penv: *mut *mut std::ffi::c_void, _version: jint,
) -> jint {
    let state = crate::state::jvm_state();
    let guard = state.lock().unwrap();
    *penv = guard.env_handle as *mut std::ffi::c_void;
    0
}
pub unsafe extern "C" fn jni_AttachCurrentThreadAsDaemon(
    _vm: *mut JavaVM, _penv: *mut *mut std::ffi::c_void, _args: *mut std::ffi::c_void,
) -> jint {
    0
}

use crate::types::*;
use crate::jni_interface::JNINativeInterface;

pub type JavaVM = *mut JavaVMAttrs;
pub type JNIEnv = *mut JNIEnvAttrs;

#[repr(C)]
pub struct JavaVMAttrs {
    pub functions: *mut JavaVMInterface,
}

#[repr(C)]
pub struct JNIEnvAttrs {
    pub functions: *mut JNINativeInterface,
}

#[repr(C)]
pub struct JavaVMInterface {
    pub reserved0: Option<unsafe extern "C" fn()>,
    pub reserved1: Option<unsafe extern "C" fn()>,
    pub reserved2: Option<unsafe extern "C" fn()>,
    pub DestroyJavaVM: Option<unsafe extern "C" fn(vm: *mut JavaVM) -> jint>,
    pub AttachCurrentThread:
        Option<unsafe extern "C" fn(vm: *mut JavaVM, penv: *mut *mut std::ffi::c_void, args: *mut std::ffi::c_void) -> jint>,
    pub DetachCurrentThread: Option<unsafe extern "C" fn(vm: *mut JavaVM) -> jint>,
    pub GetEnv:
        Option<unsafe extern "C" fn(vm: *mut JavaVM, penv: *mut *mut std::ffi::c_void, version: jint) -> jint>,
    pub AttachCurrentThreadAsDaemon:
        Option<unsafe extern "C" fn(vm: *mut JavaVM, penv: *mut *mut std::ffi::c_void, args: *mut std::ffi::c_void) -> jint>,
}

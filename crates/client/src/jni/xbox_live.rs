use std::ffi::{c_char, c_void, CStr, CString};
use std::path::Path;
use libjnivm_sys::*;

// Constants matching C++ xbox_live.h
const TICKET_OK: i32 = 0;
const TICKET_UI_INTERACTION_REQUIRED: i32 = 1;
const TICKET_UNKNOWN_ERROR: i32 = 3;

const AUTH_FLOW_OK: i32 = 0;
const AUTH_FLOW_CANCEL: i32 = 1;
const AUTH_FLOW_ERROR: i32 = 2;

extern "C" {
    fn path_helper_get_primary_data_directory() -> *const c_char;
}

fn get_iface(env: *mut JNIEnv) -> *mut JNINativeInterface {
    if env.is_null() { return std::ptr::null_mut(); }
    unsafe { *(env as *mut *mut JNINativeInterface) }
}

fn new_jstring(env: *mut JNIEnv, s: &str) -> jstring {
    let iface = get_iface(env);
    if iface.is_null() { return std::ptr::null_mut(); }
    let new_string = match unsafe { (*iface).NewStringUTF } { Some(f) => f, None => return std::ptr::null_mut() };
    let c_str = CString::new(s).unwrap_or_default();
    unsafe { new_string(env, c_str.as_ptr()) as jstring }
}

fn call_static_void_method(env: *mut JNIEnv, cls: jclass, name: &str, sig: &str, args: &mut [jvalue]) {
    let iface = get_iface(env);
    if iface.is_null() { return; }
    let get_mid = match unsafe { (*iface).GetStaticMethodID } { Some(f) => f, None => return };
    let call = match unsafe { (*iface).CallStaticVoidMethodA } { Some(f) => f, None => return };
    let name_c = CString::new(name).unwrap_or_default();
    let sig_c = CString::new(sig).unwrap_or_default();
    let mid = unsafe { get_mid(env, cls, name_c.as_ptr(), sig_c.as_ptr()) };
    if !mid.is_null() {
        unsafe { call(env, cls, mid, args.as_mut_ptr()) };
    } else {
        log::warn!("XboxInterop: method {} with sig {} not found", name, sig);
    }
}

fn get_interop_class(env: *mut JNIEnv) -> jclass {
    let iface = get_iface(env);
    if iface.is_null() { return std::ptr::null_mut(); }
    let find_class = match unsafe { (*iface).FindClass } { Some(f) => f, None => return std::ptr::null_mut() };
    unsafe { find_class(env, b"com/microsoft/xbox/idp/interop/Interop\0".as_ptr() as *const c_char) }
}

fn read_config_file_content() -> String {
    let data_dir = unsafe {
        let ptr = path_helper_get_primary_data_directory();
        if ptr.is_null() { return "{}".to_string(); }
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    };
    let config_path = Path::new(&data_dir).join("assets").join("xboxservices.config");
    std::fs::read_to_string(&config_path).unwrap_or_else(|_| "{}".to_string())
}

// ======== XboxInterop (com/microsoft/xbox/idp/interop/Interop) ========

#[no_mangle]
pub unsafe extern "C" fn Java_com_microsoft_xbox_idp_interop_Interop_getLocalStoragePath(
    env: *mut JNIEnv,
    _self: jobject,
    _context: jobject,
) -> jstring {
    let dir = unsafe {
        let ptr = path_helper_get_primary_data_directory();
        if ptr.is_null() { return new_jstring(env, ""); }
        CStr::from_ptr(ptr).to_string_lossy().into_owned()
    };
    log::info!("XboxInterop: getLocalStoragePath -> {}", dir);
    new_jstring(env, &dir)
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_microsoft_xbox_idp_interop_Interop_readConfigFile(
    env: *mut JNIEnv,
    _self: jobject,
    _context: jobject,
) -> jstring {
    let config = read_config_file_content();
    log::info!("XboxInterop: readConfigFile -> {} bytes", config.len());
    new_jstring(env, &config)
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_microsoft_xbox_idp_interop_Interop_getLocale(
    env: *mut JNIEnv,
    _self: jobject,
) -> jstring {
    new_jstring(env, "en")
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_microsoft_xbox_idp_interop_Interop_invokeMSA(
    env: *mut JNIEnv,
    self_: jobject,
    _context: jobject,
    request_code: jint,
    _is_prod: jboolean,
    cid: jstring,
) {
    let cid_str = {
        let iface = get_iface(env);
        if iface.is_null() { return; }
        let get_chars = match (*iface).GetStringUTFChars { Some(f) => f, None => return };
        let release = (*iface).ReleaseStringUTFChars;
        let c_str = get_chars(env, cid, std::ptr::null_mut());
        if c_str.is_null() { return; }
        let result = CStr::from_ptr(c_str).to_string_lossy().into_owned();
        if let Some(f) = release { f(env, cid, c_str); }
        result
    };

    log::info!("XboxInterop: invokeMSA requestCode={} cid={}", request_code, cid_str);

    if request_code == 1 {
        // Silent sign-in — stub always fails
        let cls = get_interop_class(env);
        if cls.is_null() { return; }
        ticket_callback_impl(env, cls, "", request_code, TICKET_UNKNOWN_ERROR,
            "Xbox Live not available (stub)");
    } else if request_code == 6 {
        // Sign out
        sign_out_callback_impl(env);
    } else {
        log::error!("XboxInterop: unsupported requestCode {}", request_code);
    }
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_microsoft_xbox_idp_interop_Interop_invokeAuthFlow(
    env: *mut JNIEnv,
    _self: jobject,
    user_ptr: jlong,
    _activity: jobject,
    _is_prod: jboolean,
    _sign_in_text: jstring,
) {
    log::info!("XboxInterop: invokeAuthFlow userPtr={}", user_ptr);
    let cls = get_interop_class(env);
    if cls.is_null() { return; }
    auth_flow_callback_impl(env, cls, user_ptr, AUTH_FLOW_ERROR, "");
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_microsoft_xbox_idp_interop_Interop_initCLL(
    _env: *mut JNIEnv,
    _self: jobject,
    _arg0: jobject,
    _arg1: jstring,
) {
    log::info!("XboxInterop: initCLL (stub)");
}

#[no_mangle]
pub unsafe extern "C" fn Java_com_microsoft_xbox_idp_interop_Interop_logCLL(
    _env: *mut JNIEnv,
    _self: jobject,
    _ticket: jstring,
    _name: jstring,
    _data: jstring,
) {
    log::info!("XboxInterop: logCLL (stub — event dropped)");
}

// ======== Private callback helpers (call Java static methods) ========

unsafe fn ticket_callback_impl(env: *mut JNIEnv, cls: jclass, ticket: &str,
    request_code: i32, error_code: i32, error_str: &str) {
    let j_ticket = new_jstring(env, ticket);
    let j_error = new_jstring(env, error_str);
    let mut args = [
        jvalue { l: j_ticket as jobject },
        jvalue { i: request_code },
        jvalue { i: error_code },
        jvalue { l: j_error as jobject },
    ];
    log::info!("XboxInterop: ticket_callback ticket={} req={} err={} msg={}",
        ticket, request_code, error_code, error_str);
    call_static_void_method(env, cls, "ticket_callback",
        "(Ljava/lang/String;IILjava/lang/String;)V", &mut args);
}

unsafe fn auth_flow_callback_impl(env: *mut JNIEnv, cls: jclass,
    user_ptr: jlong, status: i32, cid: &str) {
    let j_cid = new_jstring(env, cid);
    let mut args = [
        jvalue { j: user_ptr },
        jvalue { i: status },
        jvalue { l: j_cid as jobject },
    ];
    log::info!("XboxInterop: auth_flow_callback userPtr={} status={} cid={}",
        user_ptr, status, cid);
    call_static_void_method(env, cls, "auth_flow_callback",
        "(JILjava/lang/String;)V", &mut args);
}

unsafe fn sign_out_callback_impl(env: *mut JNIEnv) {
    let cls = get_interop_class(env);
    if cls.is_null() { return; }
    let mut args: [jvalue; 0] = [];
    log::info!("XboxInterop: sign_out_callback");
    call_static_void_method(env, cls, "sign_out_callback", "()V", &mut args);
}

// ======== XboxLocalStorage (com/microsoft/xboxlive/LocalStorage) ========

#[no_mangle]
pub unsafe extern "C" fn Java_com_microsoft_xboxlive_LocalStorage_getPath(
    env: *mut JNIEnv,
    _self: jobject,
    context: jobject,
) -> jstring {
    // Delegates to XboxInterop.getLocalStoragePath
    Java_com_microsoft_xbox_idp_interop_Interop_getLocalStoragePath(env, _self, context)
}

// ======== Registration ========

pub fn register(env: *mut JNIEnv) {
    let interop_methods = [
        JNINativeMethod {
            name: b"getLocalStoragePath\0".as_ptr() as *const c_char,
            signature: b"(Landroid/content/Context;)Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: Java_com_microsoft_xbox_idp_interop_Interop_getLocalStoragePath as *mut c_void,
        },
        JNINativeMethod {
            name: b"readConfigFile\0".as_ptr() as *const c_char,
            signature: b"(Landroid/content/Context;)Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: Java_com_microsoft_xbox_idp_interop_Interop_readConfigFile as *mut c_void,
        },
        JNINativeMethod {
            name: b"getLocale\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: Java_com_microsoft_xbox_idp_interop_Interop_getLocale as *mut c_void,
        },
        JNINativeMethod {
            name: b"invokeMSA\0".as_ptr() as *const c_char,
            signature: b"(Landroid/content/Context;IZLjava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_microsoft_xbox_idp_interop_Interop_invokeMSA as *mut c_void,
        },
        JNINativeMethod {
            name: b"invokeAuthFlow\0".as_ptr() as *const c_char,
            signature: b"(JLandroid/app/Activity;ZLjava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_microsoft_xbox_idp_interop_Interop_invokeAuthFlow as *mut c_void,
        },
        JNINativeMethod {
            name: b"initCLL\0".as_ptr() as *const c_char,
            signature: b"(Landroid/content/Context;Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_microsoft_xbox_idp_interop_Interop_initCLL as *mut c_void,
        },
        JNINativeMethod {
            name: b"logCLL\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: Java_com_microsoft_xbox_idp_interop_Interop_logCLL as *mut c_void,
        },
    ];

    let storage_methods = [
        JNINativeMethod {
            name: b"getPath\0".as_ptr() as *const c_char,
            signature: b"(Landroid/content/Context;)Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: Java_com_microsoft_xboxlive_LocalStorage_getPath as *mut c_void,
        },
    ];

    let interop_cls = unsafe {
        jnivm_find_class(env, b"com/microsoft/xbox/idp/interop/Interop\0".as_ptr() as *const c_char)
    };
    if !interop_cls.is_null() {
        let rc = unsafe {
            jnivm_register_natives(env, interop_cls, interop_methods.as_ptr(), interop_methods.len() as i32)
        };
        if rc != 0 {
            log::error!("xbox_live: RegisterNatives failed for Interop");
        } else {
            log::info!("xbox_live: registered {} Interop natives", interop_methods.len());
        }
    } else {
        log::warn!("xbox_live: could not find Interop class");
    }

    let storage_cls = unsafe {
        jnivm_find_class(env, b"com/microsoft/xboxlive/LocalStorage\0".as_ptr() as *const c_char)
    };
    if !storage_cls.is_null() {
        let rc = unsafe {
            jnivm_register_natives(env, storage_cls, storage_methods.as_ptr(), storage_methods.len() as i32)
        };
        if rc != 0 {
            log::error!("xbox_live: RegisterNatives failed for LocalStorage");
        } else {
            log::info!("xbox_live: registered LocalStorage natives");
        }
    } else {
        log::warn!("xbox_live: could not find LocalStorage class");
    }
}

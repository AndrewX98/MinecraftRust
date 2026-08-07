use libjnivm_sys::*;
use std::ffi::{c_char, c_void};

// ================================================================
// Helper functions
// ================================================================

fn get_iface(env: *mut JNIEnv) -> *mut JNINativeInterface {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { *(env as *mut *mut JNINativeInterface) }
}

fn new_jstring(env: *mut JNIEnv, s: &str) -> jstring {
    let iface = get_iface(env);
    if iface.is_null() {
        return std::ptr::null_mut();
    }
    let new_string = match unsafe { (*iface).NewStringUTF } {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let c_str = std::ffi::CString::new(s).unwrap_or_default();
    unsafe { new_string(env, c_str.as_ptr()) as jstring }
}

fn call_void_method(env: *mut JNIEnv, obj: jobject, name: &str, sig: &str, args: &mut [jvalue]) {
    let iface = get_iface(env);
    if iface.is_null() {
        return;
    }
    let get_class = match unsafe { (*iface).GetObjectClass } {
        Some(f) => f,
        None => return,
    };
    let get_mid = match unsafe { (*iface).GetMethodID } {
        Some(f) => f,
        None => return,
    };
    let call = match unsafe { (*iface).CallVoidMethodA } {
        Some(f) => f,
        None => return,
    };

    let cls = unsafe { get_class(env, obj) };
    let name_c = std::ffi::CString::new(name).unwrap_or_default();
    let sig_c = std::ffi::CString::new(sig).unwrap_or_default();
    let mid = unsafe { get_mid(env, cls, name_c.as_ptr(), sig_c.as_ptr()) };
    if !mid.is_null() {
        unsafe { call(env, obj, mid, args.as_mut_ptr()) };
    }
}

// ================================================================
// Store — com/mojang/minecraftpe/store/Store
// ================================================================

unsafe extern "C" fn Store_receivedLicenseResponse(_env: *mut JNIEnv, _this: jobject) -> jboolean {
    1
}

unsafe extern "C" fn Store_hasVerifiedLicense(_env: *mut JNIEnv, _this: jobject) -> jboolean {
    1
}

unsafe extern "C" fn Store_getStoreId(env: *mut JNIEnv, _this: jobject) -> jobject {
    new_jstring(env, "android.googleplay")
}

unsafe extern "C" fn Store_getProductSkuPrefix(_env: *mut JNIEnv, _this: jobject) -> jobject {
    std::ptr::null_mut()
}

unsafe extern "C" fn Store_getRealmsSkuPrefix(_env: *mut JNIEnv, _this: jobject) -> jobject {
    std::ptr::null_mut()
}

unsafe extern "C" fn Store_getExtraLicenseData(_env: *mut JNIEnv, _this: jobject) -> jobject {
    std::ptr::null_mut()
}

unsafe extern "C" fn Store_queryProducts(_env: *mut JNIEnv, _this: jobject, _skus: jobject) {}

unsafe extern "C" fn Store_purchase(_env: *mut JNIEnv, _this: jobject, _sku: jobject, _bool_val: jboolean, _extra: jobject) {}

unsafe extern "C" fn Store_acknowledgePurchase(_env: *mut JNIEnv, _this: jobject, _sku: jobject, _token: jobject) {}

unsafe extern "C" fn Store_queryPurchases(_env: *mut JNIEnv, _this: jobject) {}

unsafe extern "C" fn Store_destructor(_env: *mut JNIEnv, _this: jobject) {}

// ================================================================
// NativeStoreListener — com/mojang/minecraftpe/store/NativeStoreListener
// ================================================================

// The game C++ creates the Java NativeStoreListener via JNI NewObject with the
// (J)V constructor, passing a pointer to its own C++ StoreListener. The Rust VM
// does not interpret the Java <init> for native-created objects, so capture the
// pointer into the mStoreListener field ourselves so the game's native
// onStoreInitialized(JZ)V receives the real value.
unsafe extern "C" fn NativeStoreListener_init(
    env: *mut JNIEnv,
    this: jobject,
    ptr: i64,
    _a2: i64,
    _a3: i64,
    _a4: i64,
) {
    let iface = get_iface(env);
    if iface.is_null() {
        return;
    }
    let get_class = match unsafe { (*iface).GetObjectClass } {
        Some(f) => f,
        None => return,
    };
    let get_fid = match unsafe { (*iface).GetFieldID } {
        Some(f) => f,
        None => return,
    };
    let set_long = match unsafe { (*iface).SetLongField } {
        Some(f) => f,
        None => return,
    };
    let cls = unsafe { get_class(env, this) };
    if cls.is_null() {
        return;
    }
    let fid = unsafe { get_fid(env, cls, b"mStoreListener\0".as_ptr() as *const c_char, b"J\0".as_ptr() as *const c_char) };
    if fid.is_null() {
        return;
    }
    unsafe { set_long(env, this, fid, ptr as jlong) };
}

// ================================================================
// ExtraLicenseResponseData — com/mojang/minecraftpe/store/ExtraLicenseResponseData
// ================================================================

unsafe extern "C" fn ExtraLicenseResponseData_getValidationTime(
    _env: *mut JNIEnv,
    _this: jobject,
) -> jlong {
    60000
}

unsafe extern "C" fn ExtraLicenseResponseData_getRetryUntilTime(
    _env: *mut JNIEnv,
    _this: jobject,
) -> jlong {
    0
}

unsafe extern "C" fn ExtraLicenseResponseData_getRetryAttempts(
    _env: *mut JNIEnv,
    _this: jobject,
) -> jlong {
    0
}

// ================================================================
// NotificationListenerService — com/mojang/minecraftpe/NotificationListenerService
// ================================================================

unsafe extern "C" fn NotificationListenerService_getDeviceRegistrationToken(
    env: *mut JNIEnv,
    _this: jobject,
) -> jobject {
    new_jstring(env, "ebe97d6c-5b83-11ec-9193-9fbef390d94b")
}

// ================================================================
// StoreFactory — com/mojang/minecraftpe/store/StoreFactory
// ================================================================

unsafe extern "C" fn StoreFactory_createGooglePlayStore(
    env: *mut JNIEnv,
    _this: jobject,
    _license_key: jstring,
    store_listener: jobject,
) -> jobject {
    create_store(env, store_listener)
}

unsafe extern "C" fn StoreFactory_createAmazonAppStore(
    env: *mut JNIEnv,
    _this: jobject,
    store_listener: jobject,
) -> jobject {
    create_store(env, store_listener)
}

// Mirror C++ StoreFactory::createGooglePlayStore: allocate a Store object and
// fire storeListener->onStoreInitialized(JZ)V (the C++ Store ctor behavior),
// so the game's store init completes instead of waiting forever. The native is
// the game's own Java_com_mojang_minecraftpe_store_NativeStoreListener_*
// registered via jni_support_register_natives, so this must NOT recurse.
unsafe fn create_store(env: *mut JNIEnv, store_listener: jobject) -> jobject {
    let iface = get_iface(env);
    if iface.is_null() {
        return std::ptr::null_mut();
    }
    let find_class = match unsafe { (*iface).FindClass } {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let alloc = match unsafe { (*iface).AllocObject } {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let cls = find_class(env, b"com/mojang/minecraftpe/store/Store\0".as_ptr() as *const c_char);
    if cls.is_null() {
        return std::ptr::null_mut();
    }
    let store = alloc(env, cls);
    if store.is_null() {
        return std::ptr::null_mut();
    }
    if !store_listener.is_null() {
        let native_ptr = get_listener_native_ptr(env, store_listener);
        let mut args = [jvalue { j: native_ptr }, jvalue { z: 1 }];
        call_void_method(env, store_listener, "onStoreInitialized", "(JZ)V", &mut args);
    }
    store
}

unsafe fn get_listener_native_ptr(env: *mut JNIEnv, obj: jobject) -> jlong {
    let iface = get_iface(env);
    if iface.is_null() {
        return 0;
    }
    let get_class = match unsafe { (*iface).GetObjectClass } {
        Some(f) => f,
        None => return 0,
    };
    let get_fid = match unsafe { (*iface).GetFieldID } {
        Some(f) => f,
        None => return 0,
    };
    let get_long = match unsafe { (*iface).GetLongField } {
        Some(f) => f,
        None => return 0,
    };
    let cls = unsafe { get_class(env, obj) };
    if cls.is_null() {
        return 0;
    }
    let fid = unsafe { get_fid(env, cls, b"mStoreListener\0".as_ptr() as *const c_char, b"J\0".as_ptr() as *const c_char) };
    if fid.is_null() {
        return 0;
    }
    unsafe { get_long(env, obj, fid) }
}

// ================================================================
// Registration
// ================================================================

fn reg(env: *mut JNIEnv, class_name: &[u8], methods: &[JNINativeMethod]) {
    let cls = unsafe { jnivm_find_class(env, class_name.as_ptr() as *const c_char) };
    if cls.is_null() {
        return;
    }
    if methods.is_empty() {
        return;
    }
    unsafe {
        jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as i32);
    }
}

pub fn register_all(env: *mut JNIEnv) {
    // Store methods
    reg(
        env,
        b"com/mojang/minecraftpe/store/Store\0",
        &[
            JNINativeMethod {
                name: b"receivedLicenseResponse\0".as_ptr() as *const c_char,
                signature: b"()Z\0".as_ptr() as *const c_char,
                fnPtr: Store_receivedLicenseResponse as *mut c_void,
            },
            JNINativeMethod {
                name: b"hasVerifiedLicense\0".as_ptr() as *const c_char,
                signature: b"()Z\0".as_ptr() as *const c_char,
                fnPtr: Store_hasVerifiedLicense as *mut c_void,
            },
            JNINativeMethod {
                name: b"getStoreId\0".as_ptr() as *const c_char,
                signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
                fnPtr: Store_getStoreId as *mut c_void,
            },
            JNINativeMethod {
                name: b"getProductSkuPrefix\0".as_ptr() as *const c_char,
                signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
                fnPtr: Store_getProductSkuPrefix as *mut c_void,
            },
            JNINativeMethod {
                name: b"getRealmsSkuPrefix\0".as_ptr() as *const c_char,
                signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
                fnPtr: Store_getRealmsSkuPrefix as *mut c_void,
            },
            JNINativeMethod {
                name: b"getExtraLicenseData\0".as_ptr() as *const c_char,
                signature: b"()Lcom/mojang/minecraftpe/store/ExtraLicenseResponseData;\0".as_ptr() as *const c_char,
                fnPtr: Store_getExtraLicenseData as *mut c_void,
            },
            JNINativeMethod {
                name: b"queryProducts\0".as_ptr() as *const c_char,
                signature: b"([Ljava/lang/String;)V\0".as_ptr() as *const c_char,
                fnPtr: Store_queryProducts as *mut c_void,
            },
            JNINativeMethod {
                name: b"purchase\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;ZLjava/lang/String;)V\0".as_ptr() as *const c_char,
                fnPtr: Store_purchase as *mut c_void,
            },
            JNINativeMethod {
                name: b"acknowledgePurchase\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;Ljava/lang/String;)V\0".as_ptr() as *const c_char,
                fnPtr: Store_acknowledgePurchase as *mut c_void,
            },
            JNINativeMethod {
                name: b"queryPurchases\0".as_ptr() as *const c_char,
                signature: b"()V\0".as_ptr() as *const c_char,
                fnPtr: Store_queryPurchases as *mut c_void,
            },
            JNINativeMethod {
                name: b"destructor\0".as_ptr() as *const c_char,
                signature: b"()V\0".as_ptr() as *const c_char,
                fnPtr: Store_destructor as *mut c_void,
            },
        ],
    );

    // NativeStoreListener <init>(J)V — capture the game's StoreListener pointer
    reg(
        env,
        b"com/mojang/minecraftpe/store/NativeStoreListener\0",
        &[JNINativeMethod {
            name: b"<init>\0".as_ptr() as *const c_char,
            signature: b"(J)V\0".as_ptr() as *const c_char,
            fnPtr: NativeStoreListener_init as *mut c_void,
        }],
    );

    // ExtraLicenseResponseData methods
    reg(
        env,
        b"com/mojang/minecraftpe/store/ExtraLicenseResponseData\0",
        &[
            JNINativeMethod {
                name: b"getValidationTime\0".as_ptr() as *const c_char,
                signature: b"()J\0".as_ptr() as *const c_char,
                fnPtr: ExtraLicenseResponseData_getValidationTime as *mut c_void,
            },
            JNINativeMethod {
                name: b"getRetryUntilTime\0".as_ptr() as *const c_char,
                signature: b"()J\0".as_ptr() as *const c_char,
                fnPtr: ExtraLicenseResponseData_getRetryUntilTime as *mut c_void,
            },
            JNINativeMethod {
                name: b"getRetryAttempts\0".as_ptr() as *const c_char,
                signature: b"()J\0".as_ptr() as *const c_char,
                fnPtr: ExtraLicenseResponseData_getRetryAttempts as *mut c_void,
            },
        ],
    );

    // NotificationListenerService methods
    reg(
        env,
        b"com/mojang/minecraftpe/NotificationListenerService\0",
        &[JNINativeMethod {
            name: b"getDeviceRegistrationToken\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: NotificationListenerService_getDeviceRegistrationToken as *mut c_void,
        }],
    );

    // StoreFactory methods
    reg(
        env,
        b"com/mojang/minecraftpe/store/StoreFactory\0",
        &[
            JNINativeMethod {
                name: b"createGooglePlayStore\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;Lcom/mojang/minecraftpe/store/StoreListener;)Lcom/mojang/minecraftpe/store/Store;\0".as_ptr() as *const c_char,
                fnPtr: StoreFactory_createGooglePlayStore as *mut c_void,
            },
            JNINativeMethod {
                name: b"createAmazonAppStore\0".as_ptr() as *const c_char,
                signature: b"(Lcom/mojang/minecraftpe/store/StoreListener;)Lcom/mojang/minecraftpe/store/Store;\0".as_ptr() as *const c_char,
                fnPtr: StoreFactory_createAmazonAppStore as *mut c_void,
            },
        ],
    );

    log::info!("store: registered Store, NativeStoreListener, ExtraLicenseResponseData, NotificationListenerService, StoreFactory methods");
}

//! Phase 1 class-registration coverage (docs/PORT_JNI_SUPPORT.md).
//!
//! Classes listed in C++ `registerJniClasses()` (jni_support.cpp:190-249)
//! that were missing from the Rust `libjnivm-sys` VM. Aggressive stubs —
//! the game only touches a handful of these at runtime; the rest are
//! lookup-only. Pure additions: the game still runs on the Baron/FakeJni VM
//! until Phase 3, so nothing here changes boot behavior.

use libjnivm_sys::*;
use std::ffi::{c_char, c_void, CStr, CString};

const JNI_TRUE: jboolean = 1;
const JNI_FALSE: jboolean = 0;

extern "C" {
    fn jnivm_get_storage_dir() -> *const c_char;
}

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
    let c_str = CString::new(s).unwrap_or_default();
    unsafe { new_string(env, c_str.as_ptr()) as jstring }
}

fn read_jstring(s: jstring) -> String {
    if s.is_null() {
        return String::new();
    }
    unsafe { CStr::from_ptr(s as *const i8).to_string_lossy().into_owned() }
}

fn new_jbyte_array(data: &[u8]) -> jobject {
    let v: Vec<jbyte> = data.iter().map(|&b| b as i8).collect();
    Box::into_raw(Box::new(v)) as jobject
}

fn new_jstring_array(values: &[&str], env: *mut JNIEnv) -> jobject {
    let v: Vec<jobject> = values.iter().map(|s| new_jstring(env, s)).collect();
    Box::into_raw(Box::new(v)) as jobject
}

fn read_jbyte_array(data: jbyteArray) -> Vec<u8> {
    if data.is_null() {
        return Vec::new();
    }
    unsafe {
        let v = &*(data as *const Vec<jbyte>);
        v.iter().map(|&b| b as u8).collect()
    }
}

/// Copy a `Box<[u8]>` returned by a `*_rust` helper (out_len gives the length)
/// into a Vec and free the source allocation.
unsafe fn take_boxed_bytes(ptr: *mut u8, out_len: i32) -> Vec<u8> {
    if ptr.is_null() || out_len <= 0 {
        return Vec::new();
    }
    let slice = std::slice::from_raw_parts_mut(ptr, out_len as usize);
    let v = slice.to_vec();
    drop(Box::from_raw(slice as *mut [u8]));
    v
}

fn ensure_class(env: *mut JNIEnv, name: &[u8]) {
    let cls = unsafe { jnivm_find_class(env, name.as_ptr() as *const c_char) };
    if cls.is_null() {
        log::warn!("class_stubs: FindClass failed for {:?}", std::str::from_utf8(name));
    }
}

fn reg(env: *mut JNIEnv, class_name: &[u8], methods: &[JNINativeMethod]) {
    let cls = unsafe { jnivm_find_class(env, class_name.as_ptr() as *const c_char) };
    if cls.is_null() {
        log::warn!("class_stubs: FindClass failed for {:?}", std::str::from_utf8(class_name));
        return;
    }
    if methods.is_empty() {
        return;
    }
    let rc = unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as i32) };
    if rc != 0 {
        log::warn!(
            "class_stubs: RegisterNatives failed for {:?}",
            std::str::from_utf8(class_name)
        );
    }
}

// ================================================================
// org/fmod/FMOD — static capability queries
// ================================================================

unsafe extern "C" fn FMOD_checkInit(_env: *mut JNIEnv, _clazz: jclass) -> jboolean {
    JNI_TRUE
}

unsafe extern "C" fn FMOD_supportsAAudio(_env: *mut JNIEnv, _clazz: jclass) -> jboolean {
    JNI_FALSE
}

unsafe extern "C" fn FMOD_supportsLowLatency(_env: *mut JNIEnv, _clazz: jclass) -> jboolean {
    JNI_TRUE
}

unsafe extern "C" fn FMOD_getAssetManager(_env: *mut JNIEnv, _clazz: jclass) -> jobject {
    std::ptr::null_mut()
}

fn register_fmod_class(env: *mut JNIEnv) {
    reg(
        env,
        b"org/fmod/FMOD\0",
        &[
            JNINativeMethod {
                name: b"checkInit\0".as_ptr() as *const c_char,
                signature: b"()Z\0".as_ptr() as *const c_char,
                fnPtr: FMOD_checkInit as *mut c_void,
            },
            JNINativeMethod {
                name: b"supportsAAudio\0".as_ptr() as *const c_char,
                signature: b"()Z\0".as_ptr() as *const c_char,
                fnPtr: FMOD_supportsAAudio as *mut c_void,
            },
            JNINativeMethod {
                name: b"supportsLowLatency\0".as_ptr() as *const c_char,
                signature: b"()Z\0".as_ptr() as *const c_char,
                fnPtr: FMOD_supportsLowLatency as *mut c_void,
            },
            JNINativeMethod {
                name: b"getAssetManager\0".as_ptr() as *const c_char,
                signature: b"()Landroid/content/res/AssetManager;\0".as_ptr() as *const c_char,
                fnPtr: FMOD_getAssetManager as *mut c_void,
            },
        ],
    );
}

// ================================================================
// java/lang/ClassLoader
// ================================================================

unsafe extern "C" fn ClassLoader_loadClass(env: *mut JNIEnv, _self: jobject, name: jstring) -> jclass {
    if name.is_null() {
        return std::ptr::null_mut();
    }
    // jstrings are NUL-terminated C strings in libjnivm-sys; resolve the class
    // by name (FindClass auto-creates the class registry entry).
    let name = CStr::from_ptr(name as *const i8);
    jnivm_find_class(env, name.as_ptr())
}

// java/lang/Class.getClassLoader()Ljava/lang/ClassLoader; — the game's JNI_OnLoad
// calls this on the MainActivity class object to obtain the ClassLoader it then
// uses for ClassLoader.findClass(). Return the ClassLoader class handle.
unsafe extern "C" fn Class_getClassLoader(env: *mut JNIEnv, _self: jobject) -> jobject {
    jnivm_find_class(env, b"java/lang/ClassLoader\0".as_ptr() as *const c_char) as jobject
}

fn register_class_loader_class(env: *mut JNIEnv) {
    reg(
        env,
        b"java/lang/ClassLoader\0",
        &[
            JNINativeMethod {
                name: b"loadClass\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;)Ljava/lang/Class;\0".as_ptr() as *const c_char,
                fnPtr: ClassLoader_loadClass as *mut c_void,
            },
            JNINativeMethod {
                name: b"findClass\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;)Ljava/lang/Class;\0".as_ptr() as *const c_char,
                fnPtr: ClassLoader_loadClass as *mut c_void,
            },
        ],
    );
}

fn register_class_class(env: *mut JNIEnv) {
    reg(
        env,
        b"java/lang/Class\0",
        &[JNINativeMethod {
            name: b"getClassLoader\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/ClassLoader;\0".as_ptr() as *const c_char,
            fnPtr: Class_getClassLoader as *mut c_void,
        }],
    );
}

// ================================================================
// android/content/ContextWrapper — reuse storage-dir File logic
// ================================================================

#[repr(C)]
struct FileObject {
    path: [i8; 4096],
}

unsafe fn make_file_from_storage() -> jobject {
    let dir = jnivm_get_storage_dir();
    if dir.is_null() {
        return std::ptr::null_mut();
    }
    let path = CStr::from_ptr(dir);
    let len = path.to_bytes().len().min(4095);
    let mut fobj = Box::new(FileObject { path: [0i8; 4096] });
    for (i, &b) in path.to_bytes()[..len].iter().enumerate() {
        fobj.path[i] = b as i8;
    }
    Box::into_raw(fobj) as jobject
}

unsafe extern "C" fn ContextWrapper_getFilesDir(_env: *mut JNIEnv, _self: jobject) -> jobject {
    make_file_from_storage()
}

unsafe extern "C" fn ContextWrapper_getCacheDir(env: *mut JNIEnv, self_: jobject) -> jobject {
    ContextWrapper_getFilesDir(env, self_)
}

fn register_context_wrapper_class(env: *mut JNIEnv) {
    reg(
        env,
        b"android/content/ContextWrapper\0",
        &[
            JNINativeMethod {
                name: b"getFilesDir\0".as_ptr() as *const c_char,
                signature: b"()Ljava/io/File;\0".as_ptr() as *const c_char,
                fnPtr: ContextWrapper_getFilesDir as *mut c_void,
            },
            JNINativeMethod {
                name: b"getCacheDir\0".as_ptr() as *const c_char,
                signature: b"()Ljava/io/File;\0".as_ptr() as *const c_char,
                fnPtr: ContextWrapper_getCacheDir as *mut c_void,
            },
        ],
    );
}

// ================================================================
// com/microsoft/xal/crypto/ShaHasher — backed by rust_bridge sha256
// ================================================================

#[repr(transparent)]
struct SendPtr<T>(*mut T);
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

static SHA_CTXS: std::sync::OnceLock<std::sync::Mutex<std::collections::HashMap<usize, SendPtr<c_void>>>> =
    std::sync::OnceLock::new();

fn sha_ctxs() -> &'static std::sync::Mutex<std::collections::HashMap<usize, SendPtr<c_void>>> {
    SHA_CTXS.get_or_init(|| std::sync::Mutex::new(std::collections::HashMap::new()))
}

unsafe extern "C" fn ShaHasher_init(_env: *mut JNIEnv, this: jobject) {
    if this.is_null() {
        return;
    }
    let ctx = crate::rust_bridge::shahasher_init_rust();
    if let Ok(mut map) = sha_ctxs().lock() {
        map.insert(this as usize, SendPtr(ctx));
    }
}

unsafe extern "C" fn ShaHasher_addBytes(_env: *mut JNIEnv, this: jobject, data: jbyteArray) {
    if this.is_null() {
        return;
    }
    let bytes = read_jbyte_array(data);
    if let Ok(map) = sha_ctxs().lock() {
        if let Some(&SendPtr(ctx)) = map.get(&(this as usize)) {
            if bytes.is_empty() {
                return;
            }
            crate::rust_bridge::shahasher_add_bytes_rust(ctx, bytes.as_ptr(), bytes.len() as i32);
        }
    }
}

unsafe extern "C" fn ShaHasher_signHash(_env: *mut JNIEnv, this: jobject) -> jobject {
    if this.is_null() {
        return std::ptr::null_mut();
    }
    let ctx = match sha_ctxs().lock() {
        Ok(mut map) => match map.remove(&(this as usize)) {
            Some(SendPtr(ctx)) => ctx,
            None => return std::ptr::null_mut(),
        },
        Err(_) => return std::ptr::null_mut(),
    };
    let mut out_len: i32 = 0;
    let raw = crate::rust_bridge::shahasher_sign_hash_rust(ctx, &mut out_len);
    crate::rust_bridge::shahasher_free_rust(ctx);
    let bytes = take_boxed_bytes(raw, out_len);
    new_jbyte_array(&bytes)
}

fn register_sha_hasher_class(env: *mut JNIEnv) {
    reg(
        env,
        b"com/microsoft/xal/crypto/ShaHasher\0",
        &[
            JNINativeMethod {
                name: b"<init>\0".as_ptr() as *const c_char,
                signature: b"()V\0".as_ptr() as *const c_char,
                fnPtr: ShaHasher_init as *mut c_void,
            },
            JNINativeMethod {
                name: b"AddBytes\0".as_ptr() as *const c_char,
                signature: b"([B)V\0".as_ptr() as *const c_char,
                fnPtr: ShaHasher_addBytes as *mut c_void,
            },
            JNINativeMethod {
                name: b"SignHash\0".as_ptr() as *const c_char,
                signature: b"()[B\0".as_ptr() as *const c_char,
                fnPtr: ShaHasher_signHash as *mut c_void,
            },
        ],
    );
}

// ================================================================
// com/microsoft/xal/crypto/SecureRandom — backed by rust_bridge
// ================================================================

unsafe extern "C" fn SecureRandom_generateRandomBytes(_env: *mut JNIEnv, _clazz: jclass, bytes: jint) -> jobject {
    if bytes <= 0 {
        return new_jbyte_array(&[]);
    }
    let mut out_len: i32 = 0;
    let raw = crate::rust_bridge::securerandom_generate_bytes_rust(bytes, &mut out_len);
    let v = take_boxed_bytes(raw, out_len);
    new_jbyte_array(&v)
}

fn register_secure_random_class(env: *mut JNIEnv) {
    reg(
        env,
        b"com/microsoft/xal/crypto/SecureRandom\0",
        &[JNINativeMethod {
            name: b"GenerateRandomBytes\0".as_ptr() as *const c_char,
            signature: b"(I)[B\0".as_ptr() as *const c_char,
            fnPtr: SecureRandom_generateRandomBytes as *mut c_void,
        }],
    );
}

// ================================================================
// android/util/Base64 — backed by rust_bridge
// ================================================================

unsafe extern "C" fn Base64_decode(_env: *mut JNIEnv, _clazz: jclass, value: jstring, _flags: jint) -> jobject {
    let s = read_jstring(value);
    let mut out_len: i32 = 0;
    let raw = crate::rust_bridge::jbase64_decode_rust(s.as_ptr() as *const c_char, s.len() as i32, &mut out_len);
    let v = take_boxed_bytes(raw, out_len);
    new_jbyte_array(&v)
}

fn register_base64_class(env: *mut JNIEnv) {
    reg(
        env,
        b"android/util/Base64\0",
        &[JNINativeMethod {
            name: b"decode\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;I)[B\0".as_ptr() as *const c_char,
            fnPtr: Base64_decode as *mut c_void,
        }],
    );
}

// ================================================================
// java/util/Arrays — backed by rust_bridge
// ================================================================

unsafe extern "C" fn Arrays_copyOfRange(_env: *mut JNIEnv, _clazz: jclass, data: jbyteArray, from: jint, to: jint) -> jobject {
    if data.is_null() || to <= from {
        return new_jbyte_array(&[]);
    }
    let bytes = read_jbyte_array(data);
    let start = from.max(0) as usize;
    let len = (to as usize).saturating_sub(start);
    let mut out_len: i32 = 0;
    let raw = crate::rust_bridge::arrays_copy_of_range_rust(
        bytes.as_ptr() as *const c_void,
        start as i32,
        len as i32,
        &mut out_len,
    );
    let v = take_boxed_bytes(raw, out_len);
    new_jbyte_array(&v)
}

fn register_arrays_class(env: *mut JNIEnv) {
    reg(
        env,
        b"java/util/Arrays\0",
        &[JNINativeMethod {
            name: b"copyOfRange\0".as_ptr() as *const c_char,
            signature: b"([BII)[B\0".as_ptr() as *const c_char,
            fnPtr: Arrays_copyOfRange as *mut c_void,
        }],
    );
}

// ================================================================
// java/security/Signature + PublicKey
// ================================================================

unsafe extern "C" fn Signature_getInstance(_env: *mut JNIEnv, _clazz: jclass, _algorithm: jstring) -> jobject {
    Box::into_raw(Box::new(1u8)) as jobject
}

unsafe extern "C" fn Signature_initVerify(_env: *mut JNIEnv, _self: jobject, _key: jobject) {}

unsafe extern "C" fn Signature_verify(_env: *mut JNIEnv, _self: jobject, _data: jbyteArray) -> jboolean {
    JNI_TRUE
}

fn register_signature_class(env: *mut JNIEnv) {
    reg(
        env,
        b"java/security/Signature\0",
        &[
            JNINativeMethod {
                name: b"getInstance\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;)Ljava/security/Signature;\0".as_ptr() as *const c_char,
                fnPtr: Signature_getInstance as *mut c_void,
            },
            JNINativeMethod {
                name: b"initVerify\0".as_ptr() as *const c_char,
                signature: b"(Ljava/security/PublicKey;)V\0".as_ptr() as *const c_char,
                fnPtr: Signature_initVerify as *mut c_void,
            },
            JNINativeMethod {
                name: b"verify\0".as_ptr() as *const c_char,
                signature: b"([B)Z\0".as_ptr() as *const c_char,
                fnPtr: Signature_verify as *mut c_void,
            },
        ],
    );
}

// ================================================================
// com/microsoft/xal/browser/WebView + BrowserLaunchActivity
// ================================================================

unsafe extern "C" fn WebView_showUrl(_env: *mut JNIEnv, _clazz: jclass, _a1: i64, _a2: i64, _a3: i64, _a4: i64) {}

unsafe extern "C" fn BrowserLaunchActivity_showUrl(_env: *mut JNIEnv, _clazz: jclass, _a1: i64, _a2: i64, _a3: i64, _a4: i64) {}

fn register_webview_classes(env: *mut JNIEnv) {
    reg(
        env,
        b"com/microsoft/xal/browser/WebView\0",
        &[JNINativeMethod {
            name: b"showUrl\0".as_ptr() as *const c_char,
            signature: b"(JLandroid/content/Context;Ljava/lang/String;Ljava/lang/String;IZJ)V\0".as_ptr() as *const c_char,
            fnPtr: WebView_showUrl as *mut c_void,
        }],
    );
    reg(
        env,
        b"com/microsoft/xal/browser/BrowserLaunchActivity\0",
        &[
            JNINativeMethod {
                name: b"showUrl\0".as_ptr() as *const c_char,
                signature: b"(JLandroid/content/Context;Ljava/lang/String;Ljava/lang/String;I[Ljava/lang/String;[Ljava/lang/String;ZJ)V\0".as_ptr() as *const c_char,
                fnPtr: BrowserLaunchActivity_showUrl as *mut c_void,
            },
            JNINativeMethod {
                name: b"showUrl\0".as_ptr() as *const c_char,
                signature: b"(JLandroid/content/Context;Ljava/lang/String;Ljava/lang/String;I[Ljava/lang/String;[Ljava/lang/String;Z)V\0".as_ptr() as *const c_char,
                fnPtr: BrowserLaunchActivity_showUrl as *mut c_void,
            },
        ],
    );
}

// ================================================================
// android/accounts/AccountManager + Account
// ================================================================

unsafe extern "C" fn AccountManager_get(_env: *mut JNIEnv, _clazz: jclass, _context: jobject) -> jobject {
    Box::into_raw(Box::new(1u8)) as jobject
}

unsafe extern "C" fn AccountManager_getAccountsByType(_env: *mut JNIEnv, _self: jobject, _account_type: jstring) -> jobject {
    let v: Vec<jobject> = Vec::new();
    Box::into_raw(Box::new(v)) as jobject
}

fn register_account_classes(env: *mut JNIEnv) {
    reg(
        env,
        b"android/accounts/AccountManager\0",
        &[
            JNINativeMethod {
                name: b"get\0".as_ptr() as *const c_char,
                signature: b"(Landroid/content/Context;)Landroid/accounts/AccountManager;\0".as_ptr() as *const c_char,
                fnPtr: AccountManager_get as *mut c_void,
            },
            JNINativeMethod {
                name: b"getAccountsByType\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;)[Landroid/accounts/Account;\0".as_ptr() as *const c_char,
                fnPtr: AccountManager_getAccountsByType as *mut c_void,
            },
        ],
    );
}

// ================================================================
// com/mojang/minecraftpe/packagesource/*
// ================================================================

unsafe extern "C" fn PackageSourceFactory_createGooglePlayPackageSource(
    _env: *mut JNIEnv,
    _clazz: jclass,
    _a1: i64,
    _a2: i64,
    _a3: i64,
    _a4: i64,
) -> jobject {
    std::ptr::null_mut()
}

fn register_package_source_classes(env: *mut JNIEnv) {
    reg(
        env,
        b"com/mojang/minecraftpe/packagesource/PackageSourceFactory\0",
        &[JNINativeMethod {
            name: b"createGooglePlayPackageSource\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;Lcom/mojang/minecraftpe/packagesource/PackageSourceListener;)Lcom/mojang/minecraftpe/packagesource/PackageSource;\0".as_ptr() as *const c_char,
            fnPtr: PackageSourceFactory_createGooglePlayPackageSource as *mut c_void,
        }],
    );
}

// ================================================================
// com/microsoft/playfab/utilities/multiplayer/*
// ================================================================

unsafe extern "C" fn AndroidJniHelperMultiplayer_createUUID(env: *mut JNIEnv, _clazz: jclass) -> jstring {
    use rand_core::RngCore;
    let mut bytes = [0u8; 16];
    rand_core::OsRng.fill_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let uuid = format!(
        "{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
        bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15],
    );
    new_jstring(env, &uuid)
}

unsafe extern "C" fn EventTracerHelperMultiplayer_getPlayFabEventCommonFields(
    env: *mut JNIEnv,
    _clazz: jclass,
    _a1: i64,
    _a2: i64,
    _a3: i64,
    _a4: i64,
) -> jobject {
    new_jstring_array(
        &[
            "OSName", "Android", "OSVersion", "12", "DeviceMake", "Linux", "DeviceModel",
            "Linux", "AppName", "Minecraft", "AppVersion", "1.0.0",
        ],
        env,
    )
}

fn register_playfab_classes(env: *mut JNIEnv) {
    reg(
        env,
        b"com/microsoft/playfab/utilities/multiplayer/AndroidJniHelperMultiplayer\0",
        &[JNINativeMethod {
            name: b"createUUID\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: AndroidJniHelperMultiplayer_createUUID as *mut c_void,
        }],
    );
    reg(
        env,
        b"com/microsoft/playfab/utilities/multiplayer/EventTracerHelperMultiplayer\0",
        &[JNINativeMethod {
            name: b"getPlayFabEventCommonFields\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)[Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: EventTracerHelperMultiplayer_getPlayFabEventCommonFields as *mut c_void,
        }],
    );
}

// ================================================================
// Marker classes (no native methods — ensure the class table is populated)
// ================================================================

fn register_marker_classes(env: *mut JNIEnv) {
    ensure_class(env, b"android/app/Activity\0");
    ensure_class(env, b"android/app/NativeActivity\0");
    ensure_class(env, b"android/content/res/AssetManager\0");
    ensure_class(env, b"com/mojang/minecraftpe/store/StoreListener\0");
    ensure_class(env, b"com/mojang/minecraftpe/store/Product\0");
    ensure_class(env, b"com/mojang/minecraftpe/store/Purchase\0");
    ensure_class(env, b"java/security/PublicKey\0");
    ensure_class(env, b"android/accounts/Account\0");
    ensure_class(env, b"com/mojang/minecraftpe/packagesource/PackageSource\0");
    ensure_class(env, b"com/mojang/minecraftpe/packagesource/PackageSourceListener\0");
    ensure_class(env, b"com/mojang/minecraftpe/packagesource/NativePackageSourceListener\0");
}

// ================================================================
// Entry point
// ================================================================

pub fn register_all(env: *mut JNIEnv) {
    register_fmod_class(env);
    register_class_loader_class(env);
    register_class_class(env);
    register_context_wrapper_class(env);
    register_sha_hasher_class(env);
    register_secure_random_class(env);
    register_base64_class(env);
    register_arrays_class(env);
    register_signature_class(env);
    register_webview_classes(env);
    register_account_classes(env);
    register_package_source_classes(env);
    register_playfab_classes(env);
    register_marker_classes(env);
    log::info!("class_stubs: registered Phase-1 coverage classes with libjnivm-sys VM");
}

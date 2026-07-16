//! Pure Rust JniSupport — replaces C++ jni_support.cpp
//!
//! Uses libjnivm-sys for the JNI VM backend. Java classes are registered
//! via FindClass + RegisterNatives (standard JNI API). Each class's methods
//! are extern "C" functions implementing the expected Java behavior.

use libjnivm_sys::*;
use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::{Mutex, Condvar, OnceLock, atomic::{AtomicBool, Ordering}};

use crate::jnivm_globals::jnivm_set_text_input_handler;

// ================================================================
// Send wrapper for raw pointers
// ================================================================

#[repr(transparent)]
struct SendPtr<T>(*mut T);
unsafe impl<T> Send for SendPtr<T> {}
unsafe impl<T> Sync for SendPtr<T> {}

// ================================================================
// GameActivity struct (matching android/game_activity.h)
// ================================================================

#[repr(C)]
#[repr(C)]
struct GameActivityCallbacks {
    on_start: Option<unsafe extern "C" fn(*mut GameActivity)>,
    on_resume: Option<unsafe extern "C" fn(*mut GameActivity)>,
    on_save_instance_state: Option<unsafe extern "C" fn(*mut GameActivity, SaveInstanceStateRecallback, *mut c_void)>,
    on_pause: Option<unsafe extern "C" fn(*mut GameActivity)>,
    on_stop: Option<unsafe extern "C" fn(*mut GameActivity)>,
    on_destroy: Option<unsafe extern "C" fn(*mut GameActivity)>,
    on_window_focus_changed: Option<unsafe extern "C" fn(*mut GameActivity, bool)>,
    on_native_window_created: Option<unsafe extern "C" fn(*mut GameActivity, *mut c_void)>,
    on_native_window_resized: Option<unsafe extern "C" fn(*mut GameActivity, *mut c_void, i32, i32)>,
    on_native_window_redraw_needed: Option<unsafe extern "C" fn(*mut GameActivity, *mut c_void)>,
    on_native_window_destroyed: Option<unsafe extern "C" fn(*mut GameActivity, *mut c_void)>,
    on_configuration_changed: Option<unsafe extern "C" fn(*mut GameActivity)>,
    on_trim_memory: Option<unsafe extern "C" fn(*mut GameActivity, i32)>,
    on_touch_event: Option<unsafe extern "C" fn(*mut GameActivity, *const c_void) -> bool>,
    on_key_down: Option<unsafe extern "C" fn(*mut GameActivity, *const c_void) -> bool>,
    on_key_up: Option<unsafe extern "C" fn(*mut GameActivity, *const c_void) -> bool>,
    on_text_input_event: Option<unsafe extern "C" fn(*mut GameActivity, *const c_void)>,
    on_window_insets_changed: Option<unsafe extern "C" fn(*mut GameActivity)>,
    on_content_rect_changed: Option<unsafe extern "C" fn(*mut GameActivity, *const c_void)>,
    on_software_keyboard_visibility_changed: Option<unsafe extern "C" fn(*mut GameActivity, bool)>,
    on_editor_action: Option<unsafe extern "C" fn(*mut GameActivity, i32) -> bool>,
}

type SaveInstanceStateRecallback = Option<unsafe extern "C" fn(*const c_char, i32, *mut c_void)>;

#[repr(C)]
struct GameActivity {
    callbacks: *mut GameActivityCallbacks,
    vm: *mut JavaVM,
    env: *mut JNIEnv,
    java_game_activity: jobject,
    internal_data_path: *const c_char,
    external_data_path: *const c_char,
    sdk_version: i32,
    instance: *mut c_void,
    asset_manager: *mut c_void,
    obb_path: *const c_char,
}

type GameActivityCreateFunc = unsafe extern "C" fn(*mut GameActivity, *mut c_void, usize);

// ================================================================
// Extern C functions (from C++ wrappers)
// ================================================================

extern "C" {
    fn register_all_jnivm_classes(env: *mut JNIEnv);
    fn jnivm_set_main_window(window: *mut c_void);
    fn jnivm_set_storage_dir(dir: *const c_char);
    fn jnivm_set_asset_manager(mgr: *mut c_void);
    fn jnivm_set_stbi_load_from_memory(fn_ptr: *mut c_void);
    fn jnivm_set_stbi_image_free(fn_ptr: *mut c_void);
    // C++ wrappers for FakeJni/PathHelper/XboxLiveHelper
    fn jni_support_get_jvm(s: *mut c_void) -> *mut c_void;
    fn fake_jni_jvm_attach_library(jvm: *mut c_void, path: *const c_char);
    fn fake_jni_local_frame_create(jvm: *mut c_void) -> *mut c_void;
    fn fake_jni_local_frame_destroy(frame: *mut c_void);
    fn fake_jni_local_frame_get_env(frame: *mut c_void) -> *mut c_void;
    fn path_helper_get_primary_data_directory() -> *const c_char;
    fn xbox_live_helper_set_jvm(jvm: *mut c_void);
    fn jni_support_get_game_activity_callbacks_ptr(s: *mut c_void) -> *mut c_void;
    fn jni_support_get_java_vm_ptr(s: *mut c_void) -> *mut c_void;
    fn jni_support_get_window_ptr(s: *mut c_void) -> *mut c_void;
    fn jni_support_get_activity_ref(s: *mut c_void) -> *mut c_void;
    fn jni_support_set_game_activity_instance(s: *mut c_void, instance: *mut c_void);
    fn jni_support_get_game_activity_ptr(s: *mut c_void) -> *mut c_void;
    fn jni_support_new_cpp() -> *mut c_void;
    fn jni_support_init_activity(s: *mut c_void);
    fn jni_support_delete(s: *mut c_void);
}

// ================================================================
// JVM state
// ================================================================

struct JvmState {
    vm: SendPtr<JavaVM>,
    env: SendPtr<JNIEnv>,
}

static BARON_ENV: OnceLock<Mutex<Option<SendPtr<c_void>>>> = OnceLock::new();

pub fn set_baron_env(env: *mut JNIEnv) {
    if let Ok(mut guard) = BARON_ENV.get_or_init(|| Mutex::new(None)).lock() {
        *guard = Some(SendPtr(env as *mut c_void));
    }
}

pub fn get_baron_env() -> Option<*mut JNIEnv> {
    if let Some(m) = BARON_ENV.get() {
        if let Ok(guard) = m.lock() {
            if let Some(ref p) = *guard {
                return Some(p.0 as *mut JNIEnv);
            }
        }
    }
    None
}

fn jvm_state() -> &'static Mutex<JvmState> {
    static STATE: OnceLock<Mutex<JvmState>> = OnceLock::new();
    STATE.get_or_init(|| {
        let vm = unsafe { jnivm_create_vm() };
        let env = unsafe { jnivm_get_env(vm) };
        Mutex::new(JvmState { vm: SendPtr(vm), env: SendPtr(env) })
    })
}

fn get_env() -> *mut JNIEnv {
    jvm_state().lock().unwrap().env.0
}

fn get_vm() -> *mut JavaVM {
    jvm_state().lock().unwrap().vm.0
}

fn get_iface(env: *mut JNIEnv) -> *mut JNINativeInterface {
    if env.is_null() { return std::ptr::null_mut(); }
    unsafe {
        // env points to JNIEnvAttrs; first field is functions pointer at offset 0
        let iface = *(env as *mut *mut JNINativeInterface);
        if iface.is_null() { return std::ptr::null_mut(); }
        iface
    }
}

/// Call a JNI method through the vtable
macro_rules! jni_call {
    ($env:expr, $iface_field:ident ($($arg:expr),*)) => {{
        let env = $env;
        let iface = get_iface(env);
        if iface.is_null() { return Default::default(); }
        let f = (*iface).$iface_field;
        if f.is_none() { return Default::default(); }
        f.unwrap()(env $(, $arg)*)
    }};
}

// ================================================================
// Class registration helpers
// ================================================================

pub fn register_all_classes() {
    let env = get_env();
    uuid::register(env);
    locale::register(env);
    certificate::register(env);
    ecdsa_impl::register(env);
    crate::jnivm_class_wrappers::register_all(env);
    crate::main_activity::register(env);
    crate::jni::store::register_all(env);
    crate::jni::audio::register(env);
    crate::jni::http_client::register(env);
    crate::jni::websocket::register(env);
    crate::jni::xbox_live::register(env);
    log::info!("jni_support: registered all Java classes with libjnivm-sys VM");
}

// ================================================================
// Game exit synchronization state
// ================================================================

struct GameState {
    game_exit_val: bool,
    looper_running: bool,
}

// ================================================================
// JniSupport orchestrator
// ================================================================

struct JniSupport {
    env: SendPtr<JNIEnv>,
    vm: SendPtr<JavaVM>,
    window: SendPtr<c_void>,
    input_queue: SendPtr<c_void>,
    game_activity: SendPtr<GameActivity>,
    game_callbacks: SendPtr<GameActivityCallbacks>,
    asset_manager: SendPtr<c_void>,
    is_game_activity: bool,
    game_handle: SendPtr<c_void>,
    game_state: Mutex<GameState>,
    game_cond: Condvar,
}

static JNI_SUPPORT: OnceLock<Mutex<Option<Box<JniSupport>>>> = OnceLock::new();

fn with_support<F, R>(f: F) -> R
where
    F: FnOnce(&mut JniSupport) -> R,
    R: Default,
{
    let lock = JNI_SUPPORT.get_or_init(|| Mutex::new(None));
    let mut guard = lock.lock().unwrap();
    if let Some(ref mut s) = *guard {
        f(s)
    } else {
        log::error!("jni_support: JniSupport not initialized");
        R::default()
    }
}

// ================================================================
// Public extern "C" API
// ================================================================

#[no_mangle]
pub unsafe extern "C" fn jni_support_new() -> *mut c_void {
    // Create the libjnivm-sys VM (first call initializes it)
    let env = get_env();
    let vm = get_vm();

    // Register all classes
    register_all_classes();

    let support = Box::new(JniSupport {
        env: SendPtr(env),
        vm: SendPtr(vm),
        window: SendPtr(std::ptr::null_mut()),
        input_queue: SendPtr(std::ptr::null_mut()),
        game_activity: SendPtr(std::ptr::null_mut()),
        game_callbacks: SendPtr(std::ptr::null_mut()),
        asset_manager: SendPtr(std::ptr::null_mut()),
        is_game_activity: true,
        game_handle: SendPtr(std::ptr::null_mut()),
        game_state: Mutex::new(GameState { game_exit_val: false, looper_running: false }),
        game_cond: Condvar::new(),
    });

    let ptr = Box::into_raw(support);
    // Store in the global state for FakeLooper callbacks
    let lock = JNI_SUPPORT.get_or_init(|| Mutex::new(None));
    *lock.lock().unwrap() = Some(Box::from_raw(ptr));
    // Leak it again - the global state owns it
    let ptr = Box::into_raw(lock.lock().unwrap().take().unwrap());
    ptr as *mut c_void
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_destroy(s: *mut c_void) {
    if s.is_null() { return; }
    drop(Box::from_raw(s as *mut JniSupport));
    let lock = JNI_SUPPORT.get_or_init(|| Mutex::new(None));
    *lock.lock().unwrap() = None;
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_register_natives(
    s: *mut c_void,
    resolver: Option<unsafe extern "C" fn(*const c_char) -> *mut c_void>,
) {
    if s.is_null() || resolver.is_none() { return; }
    let support = &*(s as *const JniSupport);
    let env = support.env.0;
    let resolver = resolver.unwrap();

    // Register native methods from the game library for each known class
    let class_natives: &[(&[u8], &[(&[u8], &[u8])])] = &[
        (b"com/mojang/minecraftpe/MainActivity\0", &[
            (b"nativeRegisterThis\0", b"()V\0"),
            (b"nativeWaitCrashManagementSetupComplete\0", b"()V\0"),
            (b"nativeInitializeWithApplicationContext\0", b"(Landroid/content/Context;)V\0"),
            (b"nativeShutdown\0", b"()V\0"),
            (b"nativeUnregisterThis\0", b"()V\0"),
            (b"nativeStopThis\0", b"()V\0"),
            (b"nativeOnDestroy\0", b"()V\0"),
            (b"nativeResize\0", b"(II)V\0"),
            (b"nativeSetTextboxText\0", b"(Ljava/lang/String;II)V\0"),
            (b"nativeCaretPosition\0", b"(I)V\0"),
            (b"nativeBackPressed\0", b"()V\0"),
            (b"nativeReturnKeyPressed\0", b"()V\0"),
            (b"nativeOnPickImageSuccess\0", b"(JLjava/lang/String;)V\0"),
            (b"nativeOnPickImageCanceled\0", b"(J)V\0"),
            (b"nativeOnPickFileSuccess\0", b"(Ljava/lang/String;)V\0"),
            (b"nativeOnPickFileCanceled\0", b"()V\0"),
            (b"nativeInitializeXboxLive\0", b"(JJ)V\0"),
            (b"nativeinitializeLibHttpClient\0", b"(J)J\0"),
            (b"nativeProcessIntentUriQuery\0", b"(Ljava/lang/String;Ljava/lang/String;)V\0"),
            (b"nativeSetIntegrityToken\0", b"(Ljava/lang/String;)V\0"),
            (b"nativeRunNativeCallbackOnUiThread\0", b"(J)V\0"),
        ]),
        (b"com/mojang/minecraftpe/NetworkMonitor\0", &[
            (b"nativeUpdateNetworkStatus\0", b"(ZZZ)V\0"),
        ]),
        (b"com/mojang/minecraftpe/store/NativeStoreListener\0", &[
            (b"onStoreInitialized\0", b"(JZ)V\0"),
            (b"onPurchaseFailed\0", b"(JLjava/lang/String;)V\0"),
            (b"onQueryProductsSuccess\0", b"(J[Lcom/mojang/minecraftpe/store/Product;)V\0"),
            (b"onQueryPurchasesSuccess\0", b"(J[Lcom/mojang/minecraftpe/store/Purchase;)V\0"),
        ]),
        (b"com/mojang/minecraftpe/input/JellyBeanDeviceManager\0", &[
            (b"onInputDeviceAddedNative\0", b"(I)V\0"),
            (b"onInputDeviceRemovedNative\0", b"(I)V\0"),
        ]),
        (b"com/xbox/httpclient/HttpClientRequest\0", &[
            (b"OnRequestCompleted\0", b"(JLcom/xbox/httpclient/HttpClientResponse;)V\0"),
            (b"OnRequestFailed\0", b"(JLjava/lang/String;)V\0"),
            (b"OnRequestFailed\0", b"(JLjava/lang/String;Ljava/lang/String;Ljava/lang/String;Z)V\0"),
        ]),
        (b"com/xbox/httpclient/HttpClientWebSocket\0", &[
            (b"onMessage\0", b"(Ljava/lang/String;)V\0"),
            (b"onBinaryMessage\0", b"(Ljava/nio/ByteBuffer;)V\0"),
            (b"onOpen\0", b"()V\0"),
            (b"onClose\0", b"(I)V\0"),
            (b"onFailure\0", b"()V\0"),
        ]),
        (b"com/mojang/minecraftpe/WebView\0", &[
            (b"urlOperationSucceeded\0", b"(JLjava/lang/String;ZLjava/lang/String;)V\0"),
        ]),
        (b"com/mojang/minecraftpe/BrowserLaunchActivity\0", &[
            (b"urlOperationSucceeded\0", b"(JLjava/lang/String;ZLjava/lang/String;)V\0"),
        ]),
        (b"com/mojang/minecraftpe/NativeInputStream\0", &[
            (b"nativeRead\0", b"(JJ[BJJ)I\0"),
        ]),
        (b"com/mojang/minecraftpe/NativeOutputStream\0", &[
            (b"nativeWrite\0", b"(J[BII)V\0"),
        ]),
        (b"com/mojang/minecraftpe/NetworkObserver\0", &[
            (b"Log\0", b"(Ljava/lang/String;)V\0"),
        ]),
        (b"com/mojang/minecraftpe/PlayIntegrity\0", &[
            (b"nativePlayIntegrityComplete\0", b"()V\0"),
        ]),
    ];

    for &(class_name, entries) in class_natives {
        let cls = jnivm_find_class(env, class_name.as_ptr() as *const c_char);
        if cls.is_null() {
            log::warn!("jni_support: FindClass failed for native registration: {:?}",
                       std::str::from_utf8(class_name));
            continue;
        }

        // Build the C++ class name for symbol resolution (replace / with _)
        let class_str = std::str::from_utf8(class_name).unwrap_or("").trim_end_matches('\0');
        let cpp_class = class_str.replace('/', "_");

        let mut jni_methods: Vec<JNINativeMethod> = Vec::new();
        for &(name, sig) in entries {
            let name_str = std::str::from_utf8(name).unwrap_or("").trim_end_matches('\0');
            let sym_name = format!("Java_{}_{}", cpp_class, name_str);
            let sym_c = CString::new(sym_name.as_str()).unwrap();
            let fn_ptr = resolver(sym_c.as_ptr());
            if fn_ptr.is_null() {
                log::warn!("jni_support: Missing native symbol: {}", sym_name);
                continue;
            }
            jni_methods.push(JNINativeMethod {
                name: name.as_ptr() as *const c_char,
                signature: sig.as_ptr() as *const c_char,
                fnPtr: fn_ptr,
            });
        }

        if !jni_methods.is_empty() {
            let rc = jnivm_register_natives(env, cls, jni_methods.as_ptr(), jni_methods.len() as i32);
            if rc != 0 {
                log::error!("jni_support: RegisterNatives failed for {:?}", class_str);
            } else {
                log::info!("jni_support: registered {} natives for {}", jni_methods.len(), class_str);
            }
        }
    }

    // Store game_handle for the resolver callback
    // (already stored in JNI_GAME_HANDLE from rust_bridge.rs)
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_start_game_with_baron(
    s: *mut c_void,
    game_create_func: *mut c_void,
    game_activity_ptr: *mut c_void,
    callbacks_ptr: *mut c_void,
    asset_manager: *mut c_void,
    stbi_load: *mut c_void,
    stbi_image_free: *mut c_void,
) {
    if s.is_null() || game_create_func.is_null() { return; }
    let gameOnCreate: unsafe extern "C" fn(*mut GameActivity, *mut c_void, usize) =
        std::mem::transmute(game_create_func);
    let ga = game_activity_ptr as *mut GameActivity;
    let cpp_callbacks = callbacks_ptr as *mut GameActivityCallbacks;

    // Get Baron JVM from C++ JniSupport
    let jvm = jni_support_get_jvm(s);

    // Baron LocalFrame — MUST be created BEFORE library attachment.
    // XSAPI's JNI_OnLoad spawns background threads that access the JNI env;
    // without an active frame, concurrent env access causes SIGSEGV.
    // This matches the C++ JniSupport::startGame ordering.
    let frame = fake_jni_local_frame_create(jvm);
    let baron_env = fake_jni_local_frame_get_env(frame) as *mut JNIEnv;
    set_baron_env(baron_env);

    // Call DT_INIT and DT_INIT_ARRAY constructors for libminecraftpe.so.
    // The Rust linker explicitly skips constructors at load time because
    // they require a JNI environment that isn't available until now
    // (see load_library_internal_no_ctors comment in linker/src/lib.rs:748-753).
    // These constructors set up global state (e.g., vtable pointers, static
    // initializers) that the game expects before its code runs. Without this,
    // function pointers resolve to base+0 (ELF header) because the global
    // ctors that should have populated them never executed.
    extern "C" {
        fn linker_rust_call_init_functions(name: *const c_char) -> bool;
    }
    let game_lib_name = CString::new("libminecraftpe.so").unwrap();
    log::info!("jni_support: calling linker_rust_call_init_functions for libminecraftpe.so");
    let inits_ok = linker_rust_call_init_functions(game_lib_name.as_ptr());
    log::info!("jni_support: linker_rust_call_init_functions returned {}", inits_ok);

    // Set up MainActivity fields matching C++ startGame
    let dir = path_helper_get_primary_data_directory();
    if !dir.is_null() {
        jnivm_set_storage_dir(dir);
    }
    // C++ setters for activity stbi function pointers
    extern "C" { fn jnivm_set_stbi_load_from_memory(fn_ptr: *mut c_void); fn jnivm_set_stbi_image_free(fn_ptr: *mut c_void); }
    jnivm_set_stbi_load_from_memory(stbi_load);
    jnivm_set_stbi_image_free(stbi_image_free);
    xbox_live_helper_set_jvm(jvm);

    // Attach game libraries to Baron JVM — matches C++ JniSupport::startGame (jni_support.cpp:357-359).
    // Without this, the Baron JVM can't resolve native methods in these libraries (uses system dlsym
    // but libraries were loaded by the bionic linker). Missing this causes SIGSEGV when the game's
    // lifecycle callbacks (onStart, onNativeWindowCreated) try JNI calls through the Baron env.
    fake_jni_jvm_attach_library(jvm, b"libfmod.so\0" as *const _ as *const c_char);
    fake_jni_jvm_attach_library(jvm, b"libminecraftpe.so\0" as *const _ as *const c_char);
    fake_jni_jvm_attach_library(jvm, b"libPlayFabMultiplayer.so\0" as *const _ as *const c_char);

    // Set up GameActivity with Baron values
    // Use the C++ GameActivityCallbacks — the game will populate these
    (*ga).callbacks = cpp_callbacks;
    (*ga).vm = jni_support_get_java_vm_ptr(s) as *mut JavaVM;
    (*ga).env = baron_env;
    (*ga).asset_manager = asset_manager;
    (*ga).java_game_activity = jni_support_get_activity_ref(s);
    (*ga).sdk_version = 32;
    let internal_path = CString::new("/internal").unwrap();
    let external_path = CString::new("/external").unwrap();
    // Leak the CStrings — they need to live for the program's lifetime (matching C++ behavior)
    let internal_path = internal_path.into_raw();
    let external_path = external_path.into_raw();
    (*ga).internal_data_path = internal_path as *const c_char;
    (*ga).external_data_path = external_path as *const c_char;

    // Call nativeRegisterThis on MainActivity through Baron env
    // BEFORE gameOnCreate, matching C++ JniSupport::startGame (jni_support.cpp:410-413).
    // The game's nativeRegisterThis sets up the MainActivity binding in the game library
    // so the game thread spawned by GameActivity_onCreate can find its Java activity ref.
    // Must NOT use jni_call! macro (would return early on null) — call JNI manually.
    let iface = get_iface(baron_env);
    if !iface.is_null() {
        let iface = &*iface;
        if let Some(find_class) = iface.FindClass {
            let main_cls = find_class(baron_env,
                b"com/mojang/minecraftpe/MainActivity\0".as_ptr() as *const c_char);
            if !main_cls.is_null() {
                if let Some(get_mid) = iface.GetMethodID {
                    let register_mid = get_mid(baron_env, main_cls,
                        b"nativeRegisterThis\0".as_ptr() as *const c_char,
                        b"()V\0".as_ptr() as *const c_char);
                    if !register_mid.is_null() {
                        if let Some(call_void) = iface.CallVoidMethod {
                            log::info!("jni_support: calling nativeRegisterThis on Baron MainActivity");
                            call_void(baron_env, (*ga).java_game_activity, register_mid);
                            log::info!("jni_support: nativeRegisterThis completed");
                        }
                    }
                }
            }
        }
    }

// Call GameActivity_onCreate — game caches Baron vm/env from ga.
    // Also triggers FakeLooper::prepare → JniSupport::onWindowCreated which sets window.
    eprintln!("=== About to call gameOnCreate (GameActivity_onCreate) ===");
    gameOnCreate(ga, std::ptr::null_mut(), 0);
    eprintln!("=== gameOnCreate returned ===");

    // Copy game instance to C++ JniSupport's gameActivity for FakeLooper dispatch
    eprintln!("=== About to call jni_support_set_game_activity_instance ===");
    jni_support_set_game_activity_instance(s, (*ga).instance);
    eprintln!("=== jni_support_set_game_activity_instance returned ===");

    // Read window from C++ JniSupport (set by FakeLooper::prepare during gameOnCreate)
    eprintln!("=== About to call jni_support_get_window_ptr ===");
    let win = jni_support_get_window_ptr(s);
    eprintln!("=== Rust read window from JniSupport after gameOnCreate: {:p} ===", win);

    // Read callbacks from the C++ GameActivityCallbacks (populated by game during gameOnCreate)
    let cb = &*cpp_callbacks;

    fn fmt_ptr(opt: Option<*const c_void>) -> String {
        match opt {
            Some(p) => format!("{:p}", p),
            None => "NULL".to_string(),
        }
    }

    eprintln!("=== C++ callbacks struct: on_start={} on_native_window_created={} ===",
              fmt_ptr(cb.on_start.map(|f| f as *const c_void)), fmt_ptr(cb.on_native_window_created.map(|f| f as *const c_void)));
    eprintln!("=== GameActivity struct: vm={:p} env={:p} callbacks={:p} instance={:p} ===", 
              (*ga).vm, (*ga).env, (*ga).callbacks, (*ga).instance);
    eprintln!("=== C++ callbacks struct: on_start={} on_native_window_created={} ===",
              fmt_ptr(cb.on_start.map(|f| f as *const c_void)), fmt_ptr(cb.on_native_window_created.map(|f| f as *const c_void)));
    // Match the C++ JniSupport::startGame ordering (jni_support.cpp:421-428):
    // the game thread spawned by GameActivity_onCreate -> android_main expects
    // onStart and onNativeWindowCreated to have primed the window/lifecycle
    // state before it enters its event loop. Skipping these triggers a SEGV
    // shortly after android_main starts (file-doallocate / vtable-mismatch
    // backtrace from glibc on an uninitialized window resource).
    eprintln!("=== Rust calling onStart (fn={}) ===",
              fmt_ptr(cb.on_start.map(|f| f as *const c_void)));
    if let Some(f) = cb.on_start {
        f(ga);
    } else {
        eprintln!("=== WARNING: onStart is NULL ===");
    }
    eprintln!("=== Rust calling onNativeWindowCreated (fn={} window={:p}) ===",
              fmt_ptr(cb.on_native_window_created.map(|f| f as *const c_void)), win);
    if let Some(f) = cb.on_native_window_created {
        f(ga, win);
    } else {
        eprintln!("=== WARNING: onNativeWindowCreated is NULL ===");
    }
    eprintln!("=== Rust callbacks DONE ===");

    // Destroy LocalFrame — env pointer becomes invalid after this (matching C++ behavior)
    // C++ calls onStart/onNativeWindowCreated INSIDE the LocalFrame, so we do the same
    fake_jni_local_frame_destroy(frame);
}

// ================================================================
// Event dispatch — called from window_callbacks_stub.cpp instead of
// C++ JniSupport::sendKeyDown/sendKeyUp/sendMotionEvent
// ================================================================

#[no_mangle]
pub unsafe extern "C" fn jni_support_send_key_down(s: *mut c_void, event: *const c_void) {
    if s.is_null() { return; }
    let support = &*(s as *const JniSupport);
    let ga = support.game_activity.0;
    if ga.is_null() { return; }
    let cb = &*support.game_callbacks.0;
    if let Some(f) = cb.on_key_down {
        f(ga, event);
    }
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_send_key_up(s: *mut c_void, event: *const c_void) {
    if s.is_null() { return; }
    let support = &*(s as *const JniSupport);
    let ga = support.game_activity.0;
    if ga.is_null() { return; }
    let cb = &*support.game_callbacks.0;
    if let Some(f) = cb.on_key_up {
        f(ga, event);
    }
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_send_motion_event(s: *mut c_void, event: *const c_void) {
    if s.is_null() { return; }
    let support = &*(s as *const JniSupport);
    let ga = support.game_activity.0;
    if ga.is_null() { return; }
    let cb = &*support.game_callbacks.0;
    if let Some(f) = cb.on_touch_event {
        f(ga, event);
    }
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_create_cpp() -> *mut c_void {
    let s = jni_support_new_cpp();
    if !s.is_null() {
        jni_support_init_activity(s);
    }
    s
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_destroy_cpp(s: *mut c_void) {
    if !s.is_null() {
        jni_support_delete(s);
    }
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_start_game(
    s: *mut c_void,
    cpp_support: *mut c_void,
    game_create: *mut c_void,
    stbi_load: *mut c_void,
    stbi_image_free: *mut c_void,
) {
    if s.is_null() || game_create.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    let env = support.env.0;

    // Set stbi function pointers for C++ MainActivity wrapper
    jnivm_set_stbi_load_from_memory(stbi_load);
    jnivm_set_stbi_image_free(stbi_image_free);

    // Create MainActivity instance via JNI NewObject
    // libjnivm-sys NewObject ignores args and returns a valid dummy pointer
    let activity = jni_call!(env, NewObject(std::ptr::null_mut(), std::ptr::null_mut()));
    let activity_ref = jni_call!(env, NewGlobalRef(activity));

    // Storage dir is set inside jni_support_start_game_with_baron via path_helper_get_primary_data_directory()

    // Get C++ GameActivity and callbacks pointers — the game will populate these
    let cpp_game_activity = jni_support_get_game_activity_ptr(cpp_support) as *mut GameActivity;
    let cpp_callbacks = jni_support_get_game_activity_callbacks_ptr(cpp_support) as *mut GameActivityCallbacks;

    // Set the asset manager (FakeAssetManager instance)
    extern "C" {
        fn fake_assetmanager_get_instance() -> *mut c_void;
    }
    let am = fake_assetmanager_get_instance();
    support.asset_manager = SendPtr(am);
    jnivm_set_asset_manager(am);

    // Combined bridge call: sets up Baron VM/env on the GameActivity, calls
    // GameActivity_onCreate (so the game caches Baron vm/env, not libjnivm-sys),
    // populates C++ JniSupport callbacks, and dispatches onStart/onNativeWindowCreated.
    // All happens within a single Baron LocalFrame — matching the C++ startGame order.
    log::info!("jni_support: calling jni_support_start_game_with_baron...");
    jni_support_start_game_with_baron(
        cpp_support,
        game_create,
        cpp_game_activity as *mut c_void,
        cpp_callbacks as *mut c_void,
        am,
        stbi_load,
        stbi_image_free,
    );
    log::info!("jni_support: jni_support_start_game_with_baron returned");

    // Call nativeUpdateNetworkStatus
    let nm_cls = jni_call!(env, FindClass(b"com/mojang/minecraftpe/NetworkMonitor\0".as_ptr() as *const c_char));
    if !nm_cls.is_null() {
        let nm_mid = jni_call!(env, GetMethodID(
            nm_cls,
            b"nativeUpdateNetworkStatus\0".as_ptr() as *const c_char,
            b"(ZZZ)V\0".as_ptr() as *const c_char
        ));
        if !nm_mid.is_null() {
            let args = [jvalue { z: 1 }, jvalue { z: 1 }, jvalue { z: 1 }];
            jni_call!(env, CallStaticVoidMethodA(nm_cls, nm_mid, args.as_ptr() as *mut jvalue));
        }
    }

    // Set game activity flag on support
    support.is_game_activity = true;

    // Store pointers to C++ GameActivity and callbacks for event dispatch
    support.game_activity = SendPtr(cpp_game_activity);
    support.game_callbacks = SendPtr(cpp_callbacks);

    // Initialize the Rust TextInputHandler global (replaces C++ TextInputHandler)
    let text_handler = crate::text_input_handler::TextInputHandler::new();
    let text_handler_ptr = Box::into_raw(Box::new(text_handler)) as *mut c_void;
    jnivm_set_text_input_handler(text_handler_ptr);

    log::info!("jni_support: startGame completed");
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_set_looper_running(s: *mut c_void, running: i32) {
    if s.is_null() { return; }
    let support = &*(s as *mut JniSupport);
    let mut state = support.game_state.lock().unwrap();
    state.looper_running = running != 0;
    if running == 0 {
        support.game_cond.notify_all();
    }
}

// ================================================================
// Event handler functions called from C++ stubs
// ================================================================

#[no_mangle]
pub unsafe extern "C" fn jni_support_on_window_created(s: *mut c_void, window: *mut c_void, input_queue: *mut c_void) {
    if s.is_null() {
        log::warn!("jni_support_on_window_created: s is null!");
        return;
    }
    log::info!("jni_support_on_window_created: setting window={:p} input_queue={:p}", window, input_queue);
    let support = &mut *(s as *mut JniSupport);
    support.window = SendPtr(window);
    support.input_queue = SendPtr(input_queue);
    log::info!("jni_support_on_window_created: done");
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_on_window_closed(s: *mut c_void) {
    if s.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    let env = support.env.0;
    let cls = jni_call!(env, FindClass(b"com/mojang/minecraftpe/MainActivity\0".as_ptr() as *const c_char));
    if !cls.is_null() {
        let mid = jni_call!(env, GetMethodID(
            cls,
            b"nativeShutdown\0".as_ptr() as *const c_char,
            b"()V\0".as_ptr() as *const c_char
        ));
        if !mid.is_null() {
            jni_call!(env, CallStaticVoidMethod(cls, mid));
        }
    }
    log::info!("jni_support_on_window_closed: nativeShutdown dispatched");
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_on_window_resized(s: *mut c_void, new_width: i32, new_height: i32) {
    if s.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    let env = support.env.0;
    let cls = jni_call!(env, FindClass(b"com/mojang/minecraftpe/MainActivity\0".as_ptr() as *const c_char));
    if !cls.is_null() {
        let mid = jni_call!(env, GetMethodID(
            cls,
            b"nativeResize\0".as_ptr() as *const c_char,
            b"(II)V\0".as_ptr() as *const c_char
        ));
        if !mid.is_null() {
            let resize_args = [jvalue { i: new_width }, jvalue { i: new_height }];
            jni_call!(env, CallStaticVoidMethodA(cls, mid, resize_args.as_ptr() as *mut jvalue));
        }
    }
}

// ================================================================
// Game lifecycle — stopGame, waitForGameExit, requestExitGame
// ================================================================

unsafe fn native_call_void(env: *mut JNIEnv, cls: jclass, name: &[u8], sig: &[u8]) {
    let mid = jni_call!(env, GetMethodID(
        cls,
        name.as_ptr() as *const c_char,
        sig.as_ptr() as *const c_char
    ));
    if !mid.is_null() {
        jni_call!(env, CallStaticVoidMethod(cls, mid));
    }
}

unsafe fn stop_game(support: &mut JniSupport) {
    let env = support.env.0;
    let cls = jni_call!(env, FindClass(b"com/mojang/minecraftpe/MainActivity\0".as_ptr() as *const c_char));
    if cls.is_null() {
        log::error!("stop_game: FindClass failed");
        return;
    }

    native_call_void(env, cls, b"nativeStopThis\0", b"()V\0");
    native_call_void(env, cls, b"nativeUnregisterThis\0", b"()V\0");
    native_call_void(env, cls, b"nativeOnDestroy\0", b"()V\0");

    if !support.game_activity.0.is_null() {
        let cb = &*support.game_callbacks.0;
        let ga = support.game_activity.0;
        if let Some(f) = cb.on_pause { f(ga); }
        if let Some(f) = cb.on_stop { f(ga); }
        if let Some(f) = cb.on_destroy { f(ga); }
    }

    let mut state = support.game_state.lock().unwrap();
    state.game_exit_val = true;
    state.looper_running = false;
    support.game_cond.notify_all();

    log::info!("stop_game: game shutdown complete");
}

fn url_decode(encoded: &str) -> String {
    let mut decoded = String::with_capacity(encoded.len());
    let mut chars = encoded.chars();
    while let Some(c) = chars.next() {
        if c == '%' {
            let hi = chars.next().and_then(|c| c.to_digit(16)).unwrap_or(0);
            let lo = chars.next().and_then(|c| c.to_digit(16)).unwrap_or(0);
            decoded.push((hi as u8 * 16 + lo as u8) as char);
        } else {
            decoded.push(c);
        }
    }
    decoded
}

unsafe fn send_uri(support: &mut JniSupport, uri: &str) {
    if !uri.starts_with("minecraft://") {
        log::warn!("send_uri: not a minecraft URI: {}", uri);
        return;
    }

    let rest = &uri["minecraft://".len()..];
    let host = rest.split('/').next().unwrap_or("");
    let query = uri.find('?')
        .map(|q| url_decode(&uri[q + 1..]))
        .unwrap_or_default();

    let env = support.env.0;
    let cls = jni_call!(env, FindClass(b"com/mojang/minecraftpe/MainActivity\0".as_ptr() as *const c_char));
    if cls.is_null() { return; }
    let mid = jni_call!(env, GetMethodID(
        cls,
        b"nativeProcessIntentUriQuery\0".as_ptr() as *const c_char,
        b"(Ljava/lang/String;Ljava/lang/String;)V\0".as_ptr() as *const c_char
    ));
    if mid.is_null() { return; }

    let query_log = query.clone();
    let host_c = CString::new(host).unwrap_or_default();
    let query_c = CString::new(query).unwrap_or_default();
    let host_j = jni_call!(env, NewStringUTF(host_c.as_ptr()));
    let query_j = jni_call!(env, NewStringUTF(query_c.as_ptr()));
    if host_j.is_null() || query_j.is_null() {
        log::error!("send_uri: failed to create JNI strings");
        return;
    }
    let args = [jvalue { l: host_j }, jvalue { l: query_j }];
    jni_call!(env, CallStaticVoidMethodA(cls, mid, args.as_ptr() as *mut jvalue));
    log::info!("send_uri: dispatched host={} query={}", host, query_log);
}

unsafe fn import_file(support: &mut JniSupport, path: &str) {
    let ext = path.rsplit('.').next().unwrap_or("");
    let valid = ["mcworld", "mcpack", "mcaddon", "mctemplate"];
    if !valid.contains(&ext) {
        log::warn!("import_file: unsupported extension .{}, must be one of {:?}", ext, valid);
        return;
    }

    let tmp_dir = std::env::temp_dir();
    let file_name = path.rsplit('/').next().unwrap_or(path);
    let dest_path = tmp_dir.join(file_name);

    if path.contains('&') {
        log::warn!("import_file: path contains '&', skipping: {}", path);
        return;
    }

    match std::fs::copy(path, &dest_path) {
        Ok(_) => log::info!("import_file: copied {} to {:?}", path, dest_path),
        Err(e) => {
            log::error!("import_file: failed to copy {}: {}", path, e);
            return;
        }
    }

    let env = support.env.0;
    let cls = jni_call!(env, FindClass(b"com/mojang/minecraftpe/MainActivity\0".as_ptr() as *const c_char));
    if cls.is_null() { return; }
    let mid = jni_call!(env, GetMethodID(
        cls,
        b"nativeProcessIntentUriQuery\0".as_ptr() as *const c_char,
        b"(Ljava/lang/String;Ljava/lang/String;)V\0".as_ptr() as *const c_char
    ));
    if mid.is_null() { return; }

    let host_c = CString::new("contentIntent").unwrap();
    let combined = format!("{}&{}", path, dest_path.display());
    let query_c = CString::new(combined).unwrap_or_default();
    let host_j = jni_call!(env, NewStringUTF(host_c.as_ptr()));
    let query_j = jni_call!(env, NewStringUTF(query_c.as_ptr()));
    if host_j.is_null() || query_j.is_null() {
        log::error!("import_file: failed to create JNI strings");
        return;
    }
    let args = [jvalue { l: host_j }, jvalue { l: query_j }];
    jni_call!(env, CallStaticVoidMethodA(cls, mid, args.as_ptr() as *mut jvalue));
    log::info!("import_file: dispatched for {}", path);
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_stop_game(s: *mut c_void) {
    if s.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    stop_game(support);
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_wait_for_game_exit(s: *mut c_void) {
    if s.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    let mut state = support.game_state.lock().unwrap();
    while !state.game_exit_val {
        state = support.game_cond.wait(state).unwrap();
    }
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_request_exit_game(s: *mut c_void) {
    if s.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    {
        let mut state = support.game_state.lock().unwrap();
        state.game_exit_val = true;
        support.game_cond.notify_all();
    }
    std::thread::spawn(|| {
        with_support(|s| {
            stop_game(s);
        });
    });
    log::info!("request_exit_game: signaled");
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_send_uri(s: *mut c_void, uri: *const c_char) {
    if s.is_null() || uri.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    let uri_str = match std::ffi::CStr::from_ptr(uri).to_str() {
        Ok(s) => s,
        Err(_) => { log::error!("jni_support_send_uri: invalid UTF-8"); return; }
    };
    send_uri(support, uri_str);
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_import_file(s: *mut c_void, path: *const c_char) {
    if s.is_null() || path.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    let path_str = match std::ffi::CStr::from_ptr(path).to_str() {
        Ok(s) => s,
        Err(_) => { log::error!("jni_support_import_file: invalid UTF-8"); return; }
    };
    import_file(support, path_str);
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_on_return_key_pressed(s: *mut c_void) {
    if s.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    let env = support.env.0;
    let cls = jni_call!(env, FindClass(b"com/mojang/minecraftpe/MainActivity\0".as_ptr() as *const c_char));
    if cls.is_null() { return; }
    let mid = jni_call!(env, GetMethodID(
        cls,
        b"nativeReturnKeyPressed\0".as_ptr() as *const c_char,
        b"()V\0".as_ptr() as *const c_char
    ));
    if !mid.is_null() {
        jni_call!(env, CallStaticVoidMethod(cls, mid));
    }
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_set_game_controller_connected(s: *mut c_void, dev_id: i32, connected: bool) {
    if s.is_null() { return; }
    let support = &mut *(s as *mut JniSupport);
    let env = support.env.0;
    let cls = jni_call!(env, FindClass(b"com/mojang/minecraftpe/input/JellyBeanDeviceManager\0".as_ptr() as *const c_char));
    if cls.is_null() { return; }
    let (name, sig) = if connected {
        (b"onInputDeviceAddedNative\0" as *const u8, b"(I)V\0" as *const u8)
    } else {
        (b"onInputDeviceRemovedNative\0" as *const u8, b"(I)V\0" as *const u8)
    };
    let mid = jni_call!(env, GetStaticMethodID(
        cls,
        name as *const c_char,
        sig as *const c_char
    ));
    if !mid.is_null() {
        let args = [jvalue { i: dev_id }];
        jni_call!(env, CallStaticVoidMethodA(cls, mid, args.as_ptr() as *mut jvalue));
    }
}

#[no_mangle]
pub unsafe extern "C" fn jni_support_is_game_activity(s: *mut c_void) -> bool {
    if s.is_null() { return true; }
    let support = &*(s as *mut JniSupport);
    support.is_game_activity
}

// ================================================================
// UUID — java/util/UUID
// ================================================================

mod uuid {
    use libjnivm_sys::*;
    use std::ffi::{c_char, c_void, CString};
    use std::sync::{LazyLock, Mutex};

    #[repr(C)]
    struct UuidObject { uuid: CString }
    unsafe impl Send for UuidObject {}
    unsafe impl Sync for UuidObject {}

    static RNG: LazyLock<Mutex<Urng>> = LazyLock::new(|| Mutex::new(Urng::new()));
    struct Urng(u64);
    impl Urng {
        fn new() -> Self {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64).unwrap_or(0x1234567890abcdef);
            Urng(seed)
        }
        fn next_u32(&mut self) -> u32 {
            let mut x = self.0; x ^= x << 13; x ^= x >> 7; x ^= x << 17;
            self.0 = x; x as u32
        }
        fn fill_bytes(&mut self, buf: &mut [u8]) {
            for chunk in buf.chunks_mut(4) {
                chunk.copy_from_slice(&self.next_u32().to_le_bytes()[..chunk.len()]);
            }
        }
    }

    fn gen_uuid(hyphens: bool) -> String {
        let mut raw = [0u8; 16];
        RNG.lock().unwrap().fill_bytes(&mut raw);
        raw[6] = (raw[6] & 0x0f) | 0x40;
        raw[8] = (raw[8] & 0x3f) | 0x80;
        if hyphens {
            format!("{:02x}{:02x}{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
                raw[0],raw[1],raw[2],raw[3],raw[4],raw[5],raw[6],raw[7],raw[8],raw[9],raw[10],raw[11],raw[12],raw[13],raw[14],raw[15])
        } else {
            raw.iter().map(|b| format!("{:02x}", b)).collect()
        }
    }

    unsafe extern "C" fn uuid_randomUUID(_env: *mut JNIEnv, _clazz: jclass) -> jobject {
        let cstr = CString::new(gen_uuid(true)).unwrap_or_default();
        Box::into_raw(Box::new(UuidObject { uuid: cstr })) as jobject
    }
    unsafe extern "C" fn uuid_makeRandomUUID(_env: *mut JNIEnv, _clazz: jclass, hyphens: jboolean) -> jobject {
        let cstr = CString::new(gen_uuid(hyphens != 0)).unwrap_or_default();
        Box::into_raw(Box::new(UuidObject { uuid: cstr })) as jobject
    }
    unsafe extern "C" fn uuid_toString(env: *mut JNIEnv, this: jobject) -> jobject {
        if this.is_null() { return std::ptr::null_mut(); }
        let obj = &*(this as *const UuidObject);
        let iface = *(env as *mut *mut JNINativeInterface);
        (*iface).NewStringUTF.unwrap()(env, obj.uuid.as_ptr()) as jobject
    }

    pub fn register(env: *mut JNIEnv) {
        let methods = [
            JNINativeMethod { name: b"randomUUID\0".as_ptr() as *const c_char, signature: b"()Ljava/util/UUID;\0".as_ptr() as *const c_char, fnPtr: uuid_randomUUID as *mut c_void },
            JNINativeMethod { name: b"makeRandomUUID\0".as_ptr() as *const c_char, signature: b"(Z)Ljava/util/UUID;\0".as_ptr() as *const c_char, fnPtr: uuid_makeRandomUUID as *mut c_void },
            JNINativeMethod { name: b"toString\0".as_ptr() as *const c_char, signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char, fnPtr: uuid_toString as *mut c_void },
        ];
        let cls = unsafe { jnivm_find_class(env, b"java/util/UUID\0".as_ptr() as *const c_char) };
        if cls.is_null() { log::error!("uuid: FindClass failed"); return; }
        let rc = unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as i32) };
        if rc != 0 { log::error!("uuid: RegisterNatives failed"); }
        else { log::info!("uuid: java/util/UUID registered"); }
    }
}

// ================================================================
// Locale — java/util/Locale
// ================================================================

mod locale {
    use libjnivm_sys::*;
    use std::ffi::{c_char, c_void, CString};

    #[repr(C)]
    struct LocaleObject { name: CString }
    unsafe impl Send for LocaleObject {}
    unsafe impl Sync for LocaleObject {}

    unsafe extern "C" fn locale_getDefault(env: *mut JNIEnv, _clazz: jclass) -> jobject {
        // Prefer a stable BCP-47-ish name. Avoid raw LANG values like "C.UTF-8"
        // or Android-style "en.UTF-8" that break host std::locale / collate.
        let name = std::env::var("LANG")
            .ok()
            .filter(|s| {
                let s = s.as_str();
                s.contains('_')
                    && !s.eq_ignore_ascii_case("C")
                    && !s.eq_ignore_ascii_case("C.UTF-8")
                    && !s.eq_ignore_ascii_case("C.utf8")
                    && !s.eq_ignore_ascii_case("POSIX")
            })
            .unwrap_or_else(|| "en_US".to_string());
        // Strip encoding suffix if present ("en_US.UTF-8" → "en_US")
        let name = name.split('.').next().unwrap_or("en_US").to_string();
        let cstr = CString::new(name).unwrap_or_else(|_| CString::new("en_US").unwrap());
        Box::into_raw(Box::new(LocaleObject { name: cstr })) as jobject
    }
    unsafe extern "C" fn locale_toString(env: *mut JNIEnv, this: jobject) -> jobject {
        if this.is_null() { return std::ptr::null_mut(); }
        let obj = &*(this as *const LocaleObject);
        let iface = *(env as *mut *mut JNINativeInterface);
        (*iface).NewStringUTF.unwrap()(env, obj.name.as_ptr()) as jobject
    }

    pub fn register(env: *mut JNIEnv) {
        let methods = [
            JNINativeMethod { name: b"getDefault\0".as_ptr() as *const c_char, signature: b"()Ljava/util/Locale;\0".as_ptr() as *const c_char, fnPtr: locale_getDefault as *mut c_void },
            JNINativeMethod { name: b"toString\0".as_ptr() as *const c_char, signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char, fnPtr: locale_toString as *mut c_void },
        ];
        let cls = unsafe { jnivm_find_class(env, b"java/util/Locale\0".as_ptr() as *const c_char) };
        if cls.is_null() { log::error!("locale: FindClass failed"); return; }
        let rc = unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as i32) };
        if rc != 0 { log::error!("locale: RegisterNatives failed"); }
        else { log::info!("locale: java/util/Locale registered"); }
    }
}

// ================================================================
// Certificate stubs — java/security/cert/*, javax/net/ssl/*,
//                     java/io/InputStream, ByteArrayInputStream,
//                     StrictHostnameVerifier
// ================================================================

mod certificate {
    use libjnivm_sys::*;
    use std::ffi::{c_char, c_void};

    unsafe fn ensure_class(env: *mut JNIEnv, name: &[u8]) {
        let cls = jnivm_find_class(env, name.as_ptr() as *const c_char);
        if cls.is_null() {
            log::warn!("certificate: FindClass failed: {:?}",
                       std::str::from_utf8(name));
        }
    }

    fn reg(env: *mut JNIEnv, class_name: &[u8], methods: &[JNINativeMethod]) {
        let cls = unsafe { jnivm_find_class(env, class_name.as_ptr() as *const c_char) };
        if cls.is_null() {
            log::warn!("certificate: FindClass failed: {:?}",
                       std::str::from_utf8(class_name));
            return;
        }
        if methods.is_empty() { return; }
        let rc = unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as i32) };
        if rc != 0 {
            log::warn!("certificate: RegisterNatives failed for {:?}",
                       std::str::from_utf8(class_name));
        }
    }

    unsafe extern "C" fn cert_factory_getInstance(
        _env: *mut JNIEnv, _clazz: jclass, _s: jstring,
    ) -> jobject {
        Box::into_raw(Box::new(0u8)) as jobject
    }

    unsafe extern "C" fn cert_factory_generateCertificate(
        _env: *mut JNIEnv, _this: jobject, _stream: jobject,
    ) -> jobject {
        Box::into_raw(Box::new(0u8)) as jobject
    }

    unsafe extern "C" fn tm_factory_getInstance(
        _env: *mut JNIEnv, _clazz: jclass, _s: jstring,
    ) -> jobject {
        Box::into_raw(Box::new(0u8)) as jobject
    }

    unsafe extern "C" fn tm_factory_getTrustManagers(
        env: *mut JNIEnv, _this: jobject,
    ) -> jobject {
        let iface = *(env as *mut *mut JNINativeInterface);
        let tm_cls = jnivm_find_class(
            env,
            b"javax/net/ssl/TrustManager\0".as_ptr() as *const c_char,
        );
        if tm_cls.is_null() { return std::ptr::null_mut(); }
        let arr = match (*iface).NewObjectArray {
            Some(f) => f(env, 1, tm_cls, std::ptr::null_mut()),
            None => return std::ptr::null_mut(),
        };
        let x509 = Box::into_raw(Box::new(0u8)) as jobject;
        if let Some(f) = (*iface).SetObjectArrayElement {
            f(env, arr, 0, x509);
        }
        arr as jobject
    }

    unsafe extern "C" fn hostname_verifier_verify(
        _env: *mut JNIEnv, _this: jobject, _host: jstring, _cert: jobject,
    ) {
    }

    pub fn register(env: *mut JNIEnv) {
        // Classes without native methods — just ensure they exist
        unsafe {
            ensure_class(env, b"java/io/InputStream\0");
            ensure_class(env, b"java/io/ByteArrayInputStream\0");
            ensure_class(env, b"java/security/cert/Certificate\0");
            ensure_class(env, b"org/apache/http/conn/ssl/java/security/cert/X509Certificate\0");
            ensure_class(env, b"javax/net/ssl/TrustManager\0");
            ensure_class(env, b"javax/net/ssl/X509TrustManager\0");
        }

        // CertificateFactory
        reg(env, b"java/security/cert/CertificateFactory\0", &[
            JNINativeMethod {
                name: b"getInstance\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;)Ljava/security/cert/CertificateFactory;\0".as_ptr() as *const c_char,
                fnPtr: cert_factory_getInstance as *mut c_void,
            },
            JNINativeMethod {
                name: b"generateCertificate\0".as_ptr() as *const c_char,
                signature: b"(Ljava/io/InputStream;)Ljava/security/cert/Certificate;\0".as_ptr() as *const c_char,
                fnPtr: cert_factory_generateCertificate as *mut c_void,
            },
        ]);

        // TrustManagerFactory
        reg(env, b"javax/net/ssl/TrustManagerFactory\0", &[
            JNINativeMethod {
                name: b"getInstance\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;)Ljavax/net/ssl/TrustManagerFactory;\0".as_ptr() as *const c_char,
                fnPtr: tm_factory_getInstance as *mut c_void,
            },
            JNINativeMethod {
                name: b"getTrustManagers\0".as_ptr() as *const c_char,
                signature: b"()[Ljavax/net/ssl/TrustManager;\0".as_ptr() as *const c_char,
                fnPtr: tm_factory_getTrustManagers as *mut c_void,
            },
        ]);

        // StrictHostnameVerifier
        reg(env, b"org/apache/http/conn/ssl/StrictHostnameVerifier\0", &[
            JNINativeMethod {
                name: b"verify\0".as_ptr() as *const c_char,
                signature: b"(Ljava/lang/String;Lorg/apache/http/conn/ssl/java/security/cert/X509Certificate;)V\0".as_ptr() as *const c_char,
                fnPtr: hostname_verifier_verify as *mut c_void,
            },
        ]);

        log::info!("certificate: stubs registered for 9 classes");
    }
}

// ================================================================
// ECDSA — com/microsoft/xal/crypto/Ecdsa, EccPubKey
// ================================================================
//
// libjnivm AllocObject/NewObject only allocate a 1-byte handle. Instance
// state MUST live in side tables (same pattern as http_client.rs). Casting
// `this` to a large struct was heap-corrupting XAL during key generation.

mod ecdsa_impl {
    use libjnivm_sys::*;
    use p256::{
        ecdsa::{SigningKey, Signature, signature::Signer},
        EncodedPoint,
    };
    use rand_core::OsRng;
    use std::collections::HashMap;
    use std::ffi::{c_char, c_void, CStr, CString};
    use std::sync::{Mutex, OnceLock};

    struct EcdsaState {
        unique_id: String,
        d: [u8; 32],
        has_key: bool,
    }

    struct PubKeyState {
        x: String,
        y: String,
    }

    static ECDSA_STATES: OnceLock<Mutex<HashMap<usize, EcdsaState>>> = OnceLock::new();
    static PUBKEY_STATES: OnceLock<Mutex<HashMap<usize, PubKeyState>>> = OnceLock::new();

    fn ecdsa_states() -> &'static Mutex<HashMap<usize, EcdsaState>> {
        ECDSA_STATES.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn pubkey_states() -> &'static Mutex<HashMap<usize, PubKeyState>> {
        PUBKEY_STATES.get_or_init(|| Mutex::new(HashMap::new()))
    }

    fn signing_key_from(d: &[u8; 32]) -> Option<SigningKey> {
        let secret = p256::SecretKey::from_slice(&d[..]).ok()?;
        Some(SigningKey::from(secret))
    }

    fn pubkey_coords(sk: &SigningKey) -> Option<([u8; 32], [u8; 32])> {
        let vk = sk.verifying_key();
        let point = EncodedPoint::from(vk);
        let bytes = point.as_bytes();
        if bytes.len() < 65 || bytes[0] != 0x04 {
            return None;
        }
        let mut x = [0u8; 32];
        let mut y = [0u8; 32];
        x.copy_from_slice(&bytes[1..33]);
        y.copy_from_slice(&bytes[33..65]);
        Some((x, y))
    }

    /// Android Base64.NO_PADDING | NO_WRAP | URL_SAFE
    fn base64url(data: &[u8]) -> String {
        let b64 = util::base64::encode(data, false);
        b64.chars()
            .map(|c| match c {
                '+' => '-',
                '/' => '_',
                _ => c,
            })
            .collect()
    }

    fn new_jstring(env: *mut JNIEnv, s: &str) -> jobject {
        let iface = unsafe { *(env as *mut *mut JNINativeInterface) };
        let f = match unsafe { (*iface).NewStringUTF } {
            Some(f) => f,
            None => return std::ptr::null_mut(),
        };
        let c = CString::new(s).unwrap_or_default();
        unsafe { f(env, c.as_ptr()) as jobject }
    }

    /// libjnivm jbyteArray is a Box<Vec<jbyte>>.
    fn new_jbyte_array(data: &[u8]) -> jobject {
        let v: Vec<i8> = data.iter().map(|&b| b as i8).collect();
        Box::into_raw(Box::new(v)) as jobject
    }

    /// Opaque 1-byte handle matching libjnivm AllocObject layout.
    fn new_handle() -> jobject {
        Box::into_raw(Box::new(1u8)) as jobject
    }

    unsafe extern "C" fn ecdsa_init(_env: *mut JNIEnv, this: jobject) {
        if this.is_null() {
            return;
        }
        if let Ok(mut map) = ecdsa_states().lock() {
            map.insert(
                this as usize,
                EcdsaState {
                    unique_id: String::new(),
                    d: [0u8; 32],
                    has_key: false,
                },
            );
        }
        log::debug!("ecdsa: <init> this={:p}", this);
    }

    unsafe extern "C" fn ecdsa_generateKey(
        env: *mut JNIEnv,
        this: jobject,
        unique_id: jstring,
    ) {
        if this.is_null() {
            return;
        }
        let sk = SigningKey::random(&mut OsRng);
        let d: [u8; 32] = sk.to_bytes().into();

        let mut uid = String::new();
        if !unique_id.is_null() {
            let iface = *(env as *mut *mut JNINativeInterface);
            if let Some(get_chars) = (*iface).GetStringUTFChars {
                let chars = get_chars(env, unique_id, std::ptr::null_mut());
                if !chars.is_null() {
                    uid = CStr::from_ptr(chars).to_string_lossy().into_owned();
                    if let Some(release) = (*iface).ReleaseStringUTFChars {
                        release(env, unique_id, chars);
                    }
                }
            }
        }

        if let Ok(mut map) = ecdsa_states().lock() {
            map.insert(
                this as usize,
                EcdsaState {
                    unique_id: uid.clone(),
                    d,
                    has_key: true,
                },
            );
        }
        log::info!("ecdsa: generateKey this={:p} unique_id={}", this, uid);
    }

    unsafe extern "C" fn ecdsa_sign(
        env: *mut JNIEnv,
        this: jobject,
        data: jbyteArray,
    ) -> jobject {
        if this.is_null() || data.is_null() {
            return std::ptr::null_mut();
        }
        let (d, has_key) = {
            let map = match ecdsa_states().lock() {
                Ok(m) => m,
                Err(_) => return std::ptr::null_mut(),
            };
            match map.get(&(this as usize)) {
                Some(s) if s.has_key => (s.d, true),
                _ => {
                    log::warn!("ecdsa: sign called without key this={:p}", this);
                    return std::ptr::null_mut();
                }
            }
        };
        if !has_key {
            return std::ptr::null_mut();
        }
        let sk = match signing_key_from(&d) {
            Some(sk) => sk,
            None => return std::ptr::null_mut(),
        };

        let iface = *(env as *mut *mut JNINativeInterface);
        let elems = match (*iface).GetByteArrayElements {
            Some(f) => f(env, data, std::ptr::null_mut()),
            None => return std::ptr::null_mut(),
        };
        if elems.is_null() {
            return std::ptr::null_mut();
        }
        let len = match (*iface).GetArrayLength {
            Some(f) => f(env, data as jarray),
            None => return std::ptr::null_mut(),
        };
        let msg = std::slice::from_raw_parts(elems as *const u8, len as usize);
        let sig: Signature = sk.sign(msg);
        // Fixed 64-byte r||s (P-256 field size), matching OpenSSL BN_bn2binpad path.
        let sig_bytes = sig.to_bytes();
        if let Some(f) = (*iface).ReleaseByteArrayElements {
            f(env, data, elems, 0);
        }
        new_jbyte_array(sig_bytes.as_slice())
    }

    unsafe extern "C" fn ecdsa_getPublicKey(
        _env: *mut JNIEnv,
        this: jobject,
    ) -> jobject {
        if this.is_null() {
            return std::ptr::null_mut();
        }
        let d = {
            let map = match ecdsa_states().lock() {
                Ok(m) => m,
                Err(_) => return std::ptr::null_mut(),
            };
            match map.get(&(this as usize)) {
                Some(s) if s.has_key => s.d,
                _ => return std::ptr::null_mut(),
            }
        };
        let sk = match signing_key_from(&d) {
            Some(sk) => sk,
            None => return std::ptr::null_mut(),
        };
        let (x, y) = match pubkey_coords(&sk) {
            Some(c) => c,
            None => return std::ptr::null_mut(),
        };
        let handle = new_handle();
        if let Ok(mut map) = pubkey_states().lock() {
            map.insert(
                handle as usize,
                PubKeyState {
                    x: base64url(&x),
                    y: base64url(&y),
                },
            );
        }
        handle
    }

    unsafe extern "C" fn ecdsa_getUniqueId(
        env: *mut JNIEnv,
        this: jobject,
    ) -> jobject {
        if this.is_null() {
            return std::ptr::null_mut();
        }
        let uid = {
            let map = match ecdsa_states().lock() {
                Ok(m) => m,
                Err(_) => return std::ptr::null_mut(),
            };
            match map.get(&(this as usize)) {
                Some(s) => s.unique_id.clone(),
                None => String::new(),
            }
        };
        new_jstring(env, &uid)
    }

    unsafe extern "C" fn ecdsa_restoreKeyAndId(
        _env: *mut JNIEnv,
        _clazz: jclass,
        _ctx: jobject,
    ) -> jobject {
        // Upstream mcpelauncher also returns null — force generateKey path.
        std::ptr::null_mut()
    }

    // ---- EccPubKey methods ----
    unsafe extern "C" fn pubkey_getBase64UrlX(
        env: *mut JNIEnv,
        this: jobject,
    ) -> jobject {
        if this.is_null() {
            return std::ptr::null_mut();
        }
        let x = {
            let map = match pubkey_states().lock() {
                Ok(m) => m,
                Err(_) => return std::ptr::null_mut(),
            };
            match map.get(&(this as usize)) {
                Some(s) => s.x.clone(),
                None => return std::ptr::null_mut(),
            }
        };
        new_jstring(env, &x)
    }

    unsafe extern "C" fn pubkey_getBase64UrlY(
        env: *mut JNIEnv,
        this: jobject,
    ) -> jobject {
        if this.is_null() {
            return std::ptr::null_mut();
        }
        let y = {
            let map = match pubkey_states().lock() {
                Ok(m) => m,
                Err(_) => return std::ptr::null_mut(),
            };
            match map.get(&(this as usize)) {
                Some(s) => s.y.clone(),
                None => return std::ptr::null_mut(),
            }
        };
        new_jstring(env, &y)
    }

    fn reg(env: *mut JNIEnv, class_name: &[u8], methods: &[JNINativeMethod]) {
        let cls = unsafe { jnivm_find_class(env, class_name.as_ptr() as *const c_char) };
        if cls.is_null() {
            log::warn!(
                "ecdsa: FindClass failed: {:?}",
                std::str::from_utf8(class_name)
            );
            return;
        }
        if methods.is_empty() {
            return;
        }
        let rc =
            unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as i32) };
        if rc != 0 {
            log::warn!(
                "ecdsa: RegisterNatives failed for {:?}",
                std::str::from_utf8(class_name)
            );
        }
    }

    pub fn register(env: *mut JNIEnv) {
        reg(
            env,
            b"com/microsoft/xal/crypto/Ecdsa\0",
            &[
                JNINativeMethod {
                    name: b"<init>\0".as_ptr() as *const c_char,
                    signature: b"()V\0".as_ptr() as *const c_char,
                    fnPtr: ecdsa_init as *mut c_void,
                },
                JNINativeMethod {
                    name: b"generateKey\0".as_ptr() as *const c_char,
                    signature: b"(Ljava/lang/String;)V\0".as_ptr() as *const c_char,
                    fnPtr: ecdsa_generateKey as *mut c_void,
                },
                JNINativeMethod {
                    name: b"sign\0".as_ptr() as *const c_char,
                    signature: b"([B)[B\0".as_ptr() as *const c_char,
                    fnPtr: ecdsa_sign as *mut c_void,
                },
                JNINativeMethod {
                    name: b"getPublicKey\0".as_ptr() as *const c_char,
                    signature: b"()Lcom/microsoft/xal/crypto/EccPubKey;\0".as_ptr() as *const c_char,
                    fnPtr: ecdsa_getPublicKey as *mut c_void,
                },
                JNINativeMethod {
                    name: b"getUniqueId\0".as_ptr() as *const c_char,
                    signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
                    fnPtr: ecdsa_getUniqueId as *mut c_void,
                },
                JNINativeMethod {
                    name: b"restoreKeyAndId\0".as_ptr() as *const c_char,
                    signature: b"(Landroid/content/Context;)Lcom/microsoft/xal/crypto/Ecdsa;\0"
                        .as_ptr() as *const c_char,
                    fnPtr: ecdsa_restoreKeyAndId as *mut c_void,
                },
            ],
        );

        reg(
            env,
            b"com/microsoft/xal/crypto/EccPubKey\0",
            &[
                JNINativeMethod {
                    name: b"getBase64UrlX\0".as_ptr() as *const c_char,
                    signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
                    fnPtr: pubkey_getBase64UrlX as *mut c_void,
                },
                JNINativeMethod {
                    name: b"getBase64UrlY\0".as_ptr() as *const c_char,
                    signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
                    fnPtr: pubkey_getBase64UrlY as *mut c_void,
                },
            ],
        );

        log::info!("ecdsa: com/microsoft/xal/crypto/Ecdsa + EccPubKey registered");
    }
}


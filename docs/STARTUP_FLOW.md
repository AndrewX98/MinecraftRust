# Startup Flow

The full startup sequence from `main()` to the game running on the game thread.

## Step-by-Step

```
main()  [main.rs:15]
│
├─ 1. env_logger::init()
│     Initialize Rust logging.
│
├─ 2. capi::setup_paths()
│     → mc_setup_paths() [capi.cpp]
│     C++: pathhelper_setGameDir/DataDir/CacheDir → sets PathHelper::pathInfo
│     Result: game/data/cache directories configured
│
├─ 3. capi::init_version()
│     → mc_init_version() [capi.cpp]
│     C++: MinecraftVersion::init("com.mojang.minecraftpe", 0)
│
├─ 4. capi::get_libc_symbols_from_cpp()
│     → mc_get_libc_symbols() [capi.cpp]
│     C++: MinecraftUtils::getLibCSymbols() → merges Rust shim + C++ shim symbols
│
├─ 5. linker::load_library("libc.so", libc_syms)
│     Rust: registers libc symbols (602 Rust shim functions) with Rust linker
│
├─ 6. capi::load_core_libraries(dir)
│     → mc_load_core_libraries() [capi.cpp:131]
│     ├── linker::init() → linker_init_rust() [linker/src/lib.rs:520]
│     │   Rust: create LinkerState, register libdl symbols
│     ├── linker::load_library("libc.so", libC) → linker_load_library_rust()
│     │   Rust: register libc in linker global table
│     ├── MinecraftUtils::loadLibM()  — loads libm via glibc dlopen
│     ├── MinecraftUtils::setupHybris() — loads libz, hooks android_log
│     ├── linker::load_library() for stub libs:
│     │   libOpenSLES.so, libGLESv1_CM.so, libstdc++.so,
│     │   libGLESv2.so (stub funcs), liblog.so, libmcpelauncher_gamewindow.so
│     └── __loader_android_update_LD_LIBRARY_PATH(libDir)
│
├─ 7. capi::setup_android_hooks()
│     → mc_setup_android_hooks() [jni_bridge_stub.cpp]
│     C++ with Rust hooks: Creates FakeLooper class state, FakeAssetManager, FakeInputQueue, CorePatches
│     FakeLooper hooks (ALooper_prepare, addFd, pollAll, etc.) registered by Rust
│     via mc_register_fake_looper_hooks() [fake_looper.rs:69]
│     ├── FakeEGL::installLibrary():
│     │   ├── eglutInit() → Rust: XOpenDisplay, eglInitialize
│     │   ├── eglutCreateWindow() → Rust: XCreateWindow (NO EGL context yet)
│     │   └── fake_egl_install_library() → Rust: register libEGL.so stub,
│     │       dlopen real libEGL.so, save 10 real function pointers
│     └── CorePatches::install() → core_patches_install_impl()
│         Rust: patches AppPlatform_android23 vtable
│
├─ 8. capi::create_window_and_setup_graphics()
│     → mc_create_window_and_setup_graphics() [jni_bridge_stub.cpp]
│     C++:
│     ├── XInitThreads() (needed by Mesa EGL)
│     ├── Create actual X11 window (EGLUTWindow)
│     ├── FakeLooper::setWindow()
│     ├── MinecraftUtils::setupGLES2Symbols() — resolve real GLES2 funcs
│     ├── mc_relocate_glesv2_symbols() — replace stub GL symbols with real
│     ├── FakeEGL::saveCurrentWindowHandle() — capture real EGL handles
│     └── FakeEGL::releaseContext() — release from this thread
│
├─ 9. capi::load_minecraft()
│     → mc_load_minecraft() [jni_bridge_stub.cpp]
│     C++:
│     ├── Fill SwappyGL hooks (15 Rust stubs)
│     └── MinecraftUtils::loadMinecraftLib() → __loader_android_dlopen_ext
│         → bionic linker loads libminecraftpe.so ELF, resolves relocs
│     → CorePatches::install(handle) patches game vtable
│
├─ 10. rust_bridge::jni_set_game_handle()
│      Rust: store handle for JNI symbol resolver
│
├─ 11. capi::create_cpp_jni_support()
│      → jni_support_create_cpp() [jni_support.rs:477]
│      Rust: calls C++ jni_support_new_cpp() + jni_support_init_activity()
│      ├── new JniSupport() — C++ JniSupport with FakeJni Baron VM
│      ├── PatchJNIExceptionSafety() — wraps 10 JNI funcs with try/catch
│      ├── registerJniClasses() — 40+ Java classes registered with FakeJni VM
│      └── initActivity() — sets up activity ref
│
├─ 12. capi::set_fake_looper_jni_support()
│      → connects FakeLooper to C++ Baron JVM (for window callbacks)
│
├─ 13. capi::register_minecraft_natives_cpp()
│      → JniSupport::registerMinecraftNatives()
│      C++: RegisterNatives for 13 Java classes
│      → resolves Java_* symbols from libminecraftpe.so via symResolver
│
├─ 14. jni_support::jni_support_new()
│      Rust: creates libjnivm-sys VM
│      ├── register_all_classes():
│      │   ├── uuid::register() — java/util/UUID
│      │   ├── locale::register() — java/util/Locale
│      │   ├── certificate::register() — 9 cert/ssl stub classes
│      │   └── ecdsa_impl::register() — ECDSA crypto (p256 crate)
│      └── register_all_jnivm_classes() — C++ bridge, registers 10 classes
│          with Rust VM (File, BuildVersion, Context, MainActivity, etc.)
│
├─ 15. capi::set_fake_looper_rust_jni_support()
│      → connects FakeLooper to Rust JVM
│
├─ 16. jni_support::jni_support_register_natives()
│      Rust: RegisterNatives for 13 Java classes
│      Uses jni_resolve_symbol → mc_dlsym for Java_* symbols
│
├─ 17. capi::create_and_set_global_asset_manager()
│      → FakeAssetManager::setGlobalAssetManager(assetDir)
│
├─ 18. capi::dlsym(game_handle, "GameActivity_onCreate")
│      capi::dlsym(game_handle, "stbi_load_from_memory")
│      capi::dlsym(game_handle, "stbi_image_free")
│
├─ 19. fake_thread_mover_store_start_thread_id()
│      Rust: atomic flag for thread tracking
│
├─ 20. jni_support::jni_support_start_game(rust, cpp, game_create, stbi_load, stbi_free)
│      → jni_support_start_game() [jni_support.rs:493]
│      Rust: active start path (the C++ JniSupport::startGame() is no longer called)
│
│      ├── jnivm_set_stbi_load_from_memory(), jnivm_set_stbi_image_free()
│      ├── JNI NewObject → MainActivity instance via libjnivm-sys
│      ├── Creates GameActivity struct (Rust, leaked for program lifetime)
│      ├── **Env switch**: `(*ga).env = get_env()` (libjnivm-sys, not baron_env)
│      │   Game now dispatches all JNI calls through Rust vtable
│      ├── Sets up storage dir, asset manager
│      │
│      ├── jni_support_start_game_with_baron(cpp_support, ...) [jni_support.rs:359]
│      │   Rust: bridges to C++ FakeJni VM for Baron JNI operations
│      │   │
│      │   ├── Gets Baron JVM from C++ JniSupport
│      │   ├── vm.attachLibrary("libfmod.so")
│      │   ├── vm.attachLibrary("libminecraftpe.so")
│      │   ├── vm.attachLibrary("libPlayFabMultiplayer.so")
│      │   ├── Creates Baron LocalFrame
│      │   ├── Sets GameActivity fields (callbacks, vm, env, asset_manager, etc.)
│      │   ├── XboxLiveHelper::setJvm(jvm) — C++ helper
│      │   └── gameOnCreate(&gameActivity, nullptr, 0)
│      │       └── libminecraftpe.so's GameActivity_onCreate()
│      │           ├── Creates game thread (real pthread via libc)
│      │           ├── Game thread: ALooper_prepare → Rust fake_looper.rs
│      │           │   (Rust: initializeWindow, onWindowCreated, WindowCallbacks,
│      │           │    CorePatches, show, makeCurrent)
│      │           ├── Game thread signals readiness
│      │           └── Returns (game thread now running)
│      │   │
│      │   ├── jni_support_set_game_activity_instance()
│      │   ├── Reads callbacks from C++ JniSupport
│      │   ├── gameActivityCallbacks.onStart()
│      │   ├── gameActivityCallbacks.onNativeWindowCreated()
│      │   └── Destroys Baron LocalFrame
│      │
│      └── nativeUpdateNetworkStatus(true, true, true)
│          via libjnivm-sys JNI CallStaticVoidMethodA
│
│      ★ GAME THREAD RUNNING INDEPENDENTLY:
│      ├── Game calls eglMakeCurrent()
│      │   → fake_egl_make_current [rust_bridge.rs:940]
│      │   ├── No primary context yet → creates real EGL context + surface
│      │   │   ON THIS THREAD (avoids Mesa X11 thread affinity issue)
│      │   └── Stores per-thread context in THREAD_CONTEXTS/THREAD_SURFACES
│      ├── Game renders → eglSwapBuffers
│      │   → fake_egl_swap_buffers [rust_bridge.rs:1094]
│      │   → dispatches to real eglSwapBuffers via TLS surface
│      └── Game renders → main menu at 100% loading
│
└─ 21. fake_thread_mover_execute_main_thread()
       Rust: blocks on mpsc::recv() forever
       Main thread stays alive; game thread runs render loop
```

## Key Observations

### EGL Thread Affinity Fix (Black Screen Fix)
The `fake_egl_make_current` function (rust_bridge.rs:940) is critical:
- Real EGL context + surface are created **on the game thread**, not the main thread
- Avoids Mesa X11 thread affinity issue (EGL_BAD_ACCESS when using context from wrong thread)
- Each thread gets its own EGL surface stored in TLS (`THREAD_SURFACES`)

### Two JNI VMs — Rust Dispatch Active
- Step 11: C++ FakeJni (Baron) VM created — **legacy**, kept for `vm` operations (`AttachCurrentThread`) and `FakeLooper::onGameActivityClose`
- Step 14: Rust libjnivm-sys VM created — **primary JNI dispatch** for the game
- Step 20: `(*ga).env = get_env()` **switches game dispatch** from Baron to libjnivm-sys. All game `CallXxxMethod`/`FindClass`/`RegisterNatives` now go through the Rust vtable. `ga->vm` still points to Baron for VM-level operations.

### Dual Linker Registration
- Step 5: Rust linker registers libc symbols
- Step 6: C++ linker re-registers them via `getLibCSymbols()`

### What the Rust `jni_support_start_game()` Actually Does
The Rust version at `jni_support.rs:493` is the **active** start path. It:
1. Creates GameActivity struct with libjnivm-sys VM/env
2. Creates MainActivity via JNI NewObject
3. Sets `(*ga).env = get_env()` — **switches game JNI dispatch** to libjnivm-sys (Phase 5)
4. Bridges to C++ FakeJni via `jni_support_start_game_with_baron()` for `GameActivity_onCreate` (game caches Baron `vm`, but `env` is already switched to libjnivm-sys)
5. All 57 MainActivity methods and 9 wrapper classes handled by Rust (`main_activity.rs`, `jnivm_class_wrappers.rs`)
6. Calls lifecycle callbacks (onStart, onNativeWindowCreated) after game returns
7. C++ FakeJni still needed for: `ga->vm` operations (AttachCurrentThread), FakeLooper::onGameActivityClose, and linker compatibility

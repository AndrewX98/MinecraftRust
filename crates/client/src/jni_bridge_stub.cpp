/// Stub replacing jni_bridge.cpp for the Rust build.
/// Provides extern "C" wrappers for C++-dependent operations that are not
/// yet ported to Rust: window creation/GL setup, JNI support lifecycle, and
/// MinecraftUtils::loadMinecraftLib.
///
/// Pure orchestration (mc_jni_create, mc_jni_start_game, etc.) lives in Rust
/// in rust_bridge.rs and calls through these extern "C" wrappers.
///
/// Ported to Rust (portable subset): mc_setup_android_hooks and mc_dlsym now
/// live in capi.rs; rust_load_stub/rust_add_symbols, mc_register_android_hook,
/// the unused Rust-bridge extern block, and dead jni_support_start_game_cpp /
/// jni_support_get_text_input_handler are gone.

#include "jni/jni_support.h"
#include "xbox_live_helper.h"
#include <game_window.h>
#include <memory>

extern "C" void* mcpelauncher_dispatch_dlsym(void* handle, const char* name);
extern "C" void* mcpelauncher_dispatch_dlopen(const char* name, int flags);
extern "C" int mcpelauncher_dispatch_dlclose(void* handle);

// ============================================================
// Process-lifetime state (replaces FakeLooper statics, Phase 4)
// ============================================================
// The window token lives in Rust (`crate::game_window`, Phase 5); the C++ side
// keeps only the JniSupport pointers set once during startup
// (fake_looper_set_*_jni_support).
static JniSupport* g_jni_support = nullptr;
static void* g_rust_jni_support = nullptr;

// ============================================================
// C++ FFI helpers for Rust prepare / pollAll / addFd / attachInputQueue
// ============================================================

extern "C" void mc_set_looper_running_cpp(bool running) {
    if (g_jni_support) g_jni_support->setLooperRunning(running);
}

extern "C" void mc_jni_support_on_window_created_cpp(void* window, void* queue) {
    if (g_jni_support) g_jni_support->onWindowCreated((ANativeWindow*)window, (AInputQueue*)queue);
}

extern "C" void* mc_get_jni_support() {
    return g_jni_support;
}

extern "C" void* mc_get_rust_jni_support() {
    return g_rust_jni_support;
}

// (window helpers ported to Rust — see crate::game_window, Phase 5)
extern "C" void fake_looper_finish(void* native) {
    ANativeActivity* an = (ANativeActivity*)native;
    FakeJni::JniEnvContext ctx(*(FakeJni::Jvm *)an->vm);
    auto activity = std::dynamic_pointer_cast<MainActivity>(ctx.getJniEnv().resolveReference(an->clazz));
    if (activity) activity->quitCallback();
}

// C-linkage thunk for the GameActivity_finish hook (minecraft_load.rs).
// Previously FakeLooper::onGameActivityClose; inlined here (Phase 4).
extern "C" void fake_looper_on_game_activity_close(void* native) {
    GameActivity* ga = (GameActivity*)native;
    FakeJni::JniEnvContext ctx(*(FakeJni::Jvm *)ga->vm);
    auto activity = std::dynamic_pointer_cast<MainActivity>(ctx.getJniEnv().resolveReference(ga->javaGameActivity));
    if (activity) activity->quitCallback();
}

// ============================================================
// Window creation + GL setup (ported to Rust — crate::game_window, Phase 5)
// ============================================================
// mc_create_window_and_setup_graphics now lives in Rust and uses Rust eglut.

// ============================================================
// C++ JniSupport factory (needed by looper/window internals)
// ============================================================

// (create/destroy ported to Rust — see jni_support.rs)

extern "C" void jni_support_register_minecraft_natives_cpp(void* s, void* game_handle) {
    auto* support = (JniSupport*)s;
    static void* handle = nullptr;
    handle = game_handle;
    // Register game native methods (nativeRegisterThis, etc.) with the C++ Baron JVM.
    // This MUST be called after libminecraftpe.so is loaded but before startGame().
    // The symResolver uses linker::dlsym on the loaded game library handle.
    support->registerMinecraftNatives(+[](const char* sym) -> void* {
        return mcpelauncher_dispatch_dlsym(handle, sym);
    });
}

extern "C" void fake_looper_set_jni_support(void* support) {
    g_jni_support = (JniSupport*)support;
}

extern "C" void fake_looper_set_rust_jni_support(void* support) {
    g_rust_jni_support = support;
}

// ============================================================
// Linker symbol resolver for Rust (ported to capi.rs — Phase 12)
// ============================================================

// (bridge function ported to Rust — see minecraft_load.rs)

// ============================================================
// C++ wrappers for Rust bridge (FakeJni, PathHelper, XboxLiveHelper)
// ============================================================

extern "C" void* jni_support_get_jvm(void* s) {
    return static_cast<FakeJni::Jvm*>(((JniSupport*)s)->getJavaVM());
}

extern "C" void fake_jni_jvm_attach_library(void* jvm, const char* path) {
    // Use handle-type-agnostic dispatch wrappers: libraries owned by the Rust
    // linker (libfmod.so, libminecraftpe.so) resolve to their Rust handles so
    // JNI_OnLoad comes from the same image the game uses. Routing through the
    // C++ bionic dlopen would re-load a Rust-owned library a second time.
    static_cast<FakeJni::Jvm*>(jvm)->attachLibrary(
        path, "", {mcpelauncher_dispatch_dlopen, mcpelauncher_dispatch_dlsym, mcpelauncher_dispatch_dlclose});
}

extern "C" void* fake_jni_local_frame_create(void* jvm) {
    return new FakeJni::LocalFrame(*static_cast<FakeJni::Jvm*>(jvm));
}

extern "C" void fake_jni_local_frame_destroy(void* frame) {
    delete static_cast<FakeJni::LocalFrame*>(frame);
}

extern "C" void* fake_jni_local_frame_get_env(void* frame) {
    return &static_cast<FakeJni::LocalFrame*>(frame)->getJniEnv();
}

extern "C" void xbox_live_helper_set_jvm(void* jvm) {
    XboxLiveHelper::getInstance().setJvm(static_cast<FakeJni::Jvm*>(jvm));
}

extern "C" void* jni_support_get_game_activity_callbacks_ptr(void* s) {
    return &((JniSupport*)s)->getGameActivityCallbacks();
}

extern "C" void* jni_support_get_java_vm_ptr(void* s) {
    return ((JniSupport*)s)->getJavaVM();
}

extern "C" void* jni_support_get_window_ptr(void* s) {
    return ((JniSupport*)s)->getWindow();
}

extern "C" void* jni_support_get_activity_ref(void* s) {
    return ((JniSupport*)s)->getActivityRef();
}

extern "C" void jni_support_set_game_activity_instance(void* s, void* instance) {
    ((JniSupport*)s)->getGameActivity()->instance = instance;
}

extern "C" void* jni_support_get_game_activity_ptr(void* s) {
    return ((JniSupport*)s)->getGameActivity();
}

extern "C" void* jni_support_new_cpp() {
    return new JniSupport();
}

extern "C" void jni_support_init_activity(void* s) {
    ((JniSupport*)s)->initActivity();
}

/// Set Baron FakeJni MainActivity::storageDirectory (used by getExternalStoragePath /
/// getFilesDir). The Rust path previously only updated jnivm_set_storage_dir, so
/// AppPlatform saw CurrentFileStoragePath = ''.
extern "C" void jni_support_set_activity_storage_dir(void* s, const char* dir) {
    if (!s || !dir) return;
    ((JniSupport*)s)->setActivityStorageDir(dir);
}

extern "C" void jni_support_delete(void* s) {
    delete (JniSupport*)s;
}


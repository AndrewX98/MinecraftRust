/// Stub replacing jni_bridge.cpp for the Rust build.
/// Provides extern "C" wrappers for C++-dependent operations that are not
/// yet ported to Rust: android hooks, window creation/GL setup, JNI support
/// lifecycle, and MinecraftUtils::loadMinecraftLib.
///
/// Pure orchestration (mc_jni_create, mc_jni_start_game, etc.) lives in Rust
/// in rust_bridge.rs and calls through these extern "C" wrappers.

#include "jni/jni_support.h"
#include "fake_egl.h"
#include "fake_audio.h"
#include "xbox_live_helper.h"
#include <game_window.h>
#include <log.h>
#include <minecraft/imported/android_symbols.h>
#include "splitscreen_patch.h"
#include "shader_error_patch.h"
#include "main.h"
#include <cstdio>
#include <memory>

extern "C" void* mcpelauncher_dispatch_dlsym(void* handle, const char* name);
extern "C" void* mcpelauncher_dispatch_dlopen(const char* name, int flags);
extern "C" int mcpelauncher_dispatch_dlclose(void* handle);



#include <dlfcn.h>

#include <vector>
#include <string>
#include <unordered_map>

// Forward declare linker types/functions
struct mcpelauncher_hook_t {
    const char* name;
    void* value;
};
namespace linker {
    void* dlopen(const char* name, int flags);
    void* dlsym(void* handle, const char* symbol);
    int dlclose_unlocked(void* handle);
    void* load_library(const char* name, const std::unordered_map<std::string, void*>& symbols);
}

// Rust linker FFI bridge — register stubs with the Rust linker
extern "C" size_t linker_load_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);
extern "C" void linker_add_symbols_to_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);

static void rust_load_stub(const char* name, const std::unordered_map<std::string, void*>& syms) {
    size_t n = syms.size();
    if (n == 0) {
        linker_load_library_rust(name, nullptr, nullptr, 0);
        return;
    }
    std::vector<const char*> keys(n);
    std::vector<void*> vals(n);
    size_t i = 0;
    for (auto& [k, v] : syms) {
        keys[i] = k.c_str();
        vals[i] = v;
        i++;
    }
    linker_load_library_rust(name, keys.data(), vals.data(), n);
}

static void rust_add_symbols(const char* name, const std::unordered_map<std::string, void*>& syms) {
    size_t n = syms.size();
    if (n == 0) return;
    std::vector<const char*> keys(n);
    std::vector<void*> vals(n);
    size_t i = 0;
    for (auto& [k, v] : syms) {
        keys[i] = k.c_str();
        vals[i] = v;
        i++;
    }
    linker_add_symbols_to_library_rust(name, keys.data(), vals.data(), n);
}

struct MinecraftUtils {
    static void* loadMinecraftLib(void* showMousePointerCallback,
                                  void* hideMousePointerCallback,
                                  void* fullscreenCallback,
                                  void* closeCallback,
                                  std::vector<mcpelauncher_hook_t> hooks);
};

// Rust bridge functions
extern "C" {
    int fake_anativewindow_getwidth(void*);
    int fake_anativewindow_getheight(void*);
    void fake_swappygl_fill_hooks(mcpelauncher_hook_t* hooks, size_t count);
    void fake_thread_mover_store_start_thread_id();
    void fake_thread_mover_execute_main_thread();
    void core_patches_show_mouse_pointer();
    void core_patches_hide_mouse_pointer();
    void core_patches_set_fullscreen(void*, int);
    void core_patches_install(void* handle);
    void window_callbacks_load_gamepad_mappings();
    void mc_register_game_window_symbols();
}

// ============================================================
// Android hooks setup (uses C++ unordered_map + hybris hooks)
// ============================================================

// Wrapper to add a single entry to the android sym map from Rust
extern "C" void mc_register_android_hook(void* map, const char* name, void* fn) {
    ((std::unordered_map<std::string, void*>*)map)->insert({name, fn});
}

// Rust registers FakeLooper hooks via mc_register_fake_looper_hooks
extern "C" void mc_register_fake_looper_hooks(void* map);
// Rust registers FakeInputQueue hooks via mc_register_fake_input_queue_hooks
extern "C" void mc_register_fake_input_queue_hooks(void* map);
// Rust registers FakeAssetManager hooks via mc_register_fake_asset_manager_hooks
extern "C" void mc_register_fake_asset_manager_hooks(void* map);

extern "C" void mc_setup_android_hooks() {
    std::unordered_map<std::string, void*> android_syms;

    mc_register_fake_asset_manager_hooks(&android_syms);
    mc_register_fake_looper_hooks(&android_syms);
    android_syms["ANativeWindow_getWidth"] = (void*)fake_anativewindow_getwidth;
    android_syms["ANativeWindow_getHeight"] = (void*)fake_anativewindow_getheight;
    mc_register_fake_input_queue_hooks(&android_syms);

    // APerformanceHint stubs (BIND_NOW requires non-null GOT entries)
    android_syms["APerformanceHint_getManager"] = (void*)+[]() -> void* { return nullptr; };
    android_syms["APerformanceHint_createSession"] = (void*)+[](void*, int, long) -> void* { return nullptr; };
    android_syms["APerformanceHint_closeSession"] = (void*)+[](void*) {};
    android_syms["APerformanceHint_reportActualWorkDuration"] = (void*)+[](void*, long) {};

    for (const char** p = android_symbols; *p != nullptr; p++) {
        android_syms.insert({*p, (void*)+[](void) -> int { return 0; }});
    }

    // Phase 2: Rust-only stub registration for libandroid.so (no C++ dlopen consumer).
    rust_load_stub("libandroid.so", android_syms);

    // FMOD setOutput is stubbed to keep AAudio; FMOD then dlopen's libaaudio.so
    // and calls AAudio_* symbols. Without this shim, do_dlopen fails or the
    // Streaming Pool thread SIGSEGVs on null AAudio function pointers.
    // The Rust linker stub handles this: linker_rust_dlopen_fmod overrides FMOD's
    // relocations and find_library finds the Rust stub (no C++ soinfo involved).
    {
        std::unordered_map<std::string, void*> audio_syms;
        FakeAudio::initHybrisHooks(audio_syms);
        rust_load_stub("libaaudio.so", audio_syms);
    }
    {
        std::unordered_map<std::string, void*> audio_syms;
        FakeAudio::initHybrisHooks(audio_syms);
        rust_load_stub("libaaudio.so.2", audio_syms);
    }

    mc_register_game_window_symbols();
}

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

extern "C" void fake_looper_splitscreen_patch_gl_created() {
    SplitscreenPatch::onGLContextCreated();
}

extern "C" void fake_looper_shader_error_patch_gl_created() {
    ShaderErrorPatch::onGLContextCreated();
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

extern "C" void jni_support_start_game_cpp(void* s, void* game_on_create, void* stbi_load, void* stbi_image_free) {
    auto* support = (JniSupport*)s;
    // Use the C++ startGame which properly sets up the JNI environment
    support->startGame(nullptr, (GameActivity_createFunc*)game_on_create, stbi_load, stbi_image_free);
}

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
// Linker symbol resolver for Rust
// ============================================================

extern "C" void* mc_dlsym(void* handle, const char* symbol) {
    return mcpelauncher_dispatch_dlsym(handle, symbol);
}

// (bridge function ported to Rust — see minecraft_load.rs)

// ============================================================
// C-linkage wrapper for eglSwapBuffers (called from Rust)
// ============================================================

extern "C" int mc_egl_swap_buffers(void* display, void* surface) {
    return fake_egl::eglSwapBuffers((EGLDisplay)display, (EGLSurface)surface);
}

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

// ============================================================
// JniSupport bridge functions (void* → JniSupport*)
// ============================================================

extern "C" void* jni_support_get_text_input_handler(void*) {
    // Return the Rust global TextInputHandler instead of C++ member
    extern void* jnivm_get_text_input_handler();
    return jnivm_get_text_input_handler();
}

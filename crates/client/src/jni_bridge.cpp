#include "jni/jni_support.h"
#include "jni/main_activity.h"
#include "fake_assetmanager.h"
#include "fake_looper.h"
#include "fake_inputqueue.h"
#include "fake_egl.h"
#include "core_patches.h"
#include <game_window.h>
#include <game_window_manager.h>
#include <log.h>
#include <minecraft/imported/android_symbols.h>
#include <cstdio>
#include <exception>
#include <dlfcn.h>

extern "C" unsigned long eglutGetWindowHandle();

// Forward declare linker types/functions — avoids pulling in bionic headers (GCC 16 conflict).
struct mcpelauncher_hook_t {
    const char* name;
    void* value;
};
namespace linker {
    void* dlsym(void* handle, const char* symbol);
    void* load_library(const char* name, const std::unordered_map<std::string, void*>& symbols);
}

struct MinecraftUtils {
    static void* loadMinecraftLib(void* showMousePointerCallback,
                                  void* hideMousePointerCallback,
                                  void* fullscreenCallback,
                                  void* closeCallback,
                                  std::vector<mcpelauncher_hook_t> hooks);
};

// Declared in capi.cpp — relocates the stub libGLESv2.so soinfo with real symbols.
extern "C" void mc_relocate_glesv2_symbols(void* (*resolver)(const char*));

// Rust bridge functions (defined in rust_bridge.rs)
extern "C" {
    int fake_anativewindow_getwidth(void*);
    int fake_anativewindow_getheight(void*);
    void fake_swappygl_fill_hooks(mcpelauncher_hook_t* hooks, size_t count);
    void fake_thread_mover_store_start_thread_id();
    void fake_thread_mover_execute_main_thread();
    void core_patches_show_mouse_pointer();
    void core_patches_hide_mouse_pointer();
    void core_patches_set_fullscreen(void*, bool);
    void core_patches_install(void* handle);
    bool core_patches_is_mouse_locked();
    void core_patches_set_pending_delayed_paste();
}

/// Set up android hooks and register libandroid.so with them.
/// Must be called AFTER mc_load_core_libraries but BEFORE mc_load_minecraft,
/// so the game library's relocations resolve to real implementations.
extern "C" void mc_setup_android_hooks() {
    std::unordered_map<std::string, void*> android_syms;

    // Register real hooks for all android subsystems
    FakeAssetManager::initHybrisHooks(android_syms);
    FakeLooper::initHybrisHooks(android_syms);
    android_syms["ANativeWindow_getWidth"] = (void*)fake_anativewindow_getwidth;
    android_syms["ANativeWindow_getHeight"] = (void*)fake_anativewindow_getheight;
    FakeInputQueue::initHybrisHooks(android_syms);

    // APerformanceHint_* are WEAK UND in the game (Android NDK performance hints).
    // No library provides them on Linux; with BIND_NOW the PLT GOT entries are 0,
    // causing SIGSEGV when the PLT trampoline jumps through them. Provide stubs
    // that return NULL/0 so the game's null-checks skip the usage.
    android_syms["APerformanceHint_getManager"] = (void*)+[]() -> void* { return nullptr; };
    android_syms["APerformanceHint_createSession"] = (void*)+[](void*, int, long) -> void* { return nullptr; };
    android_syms["APerformanceHint_closeSession"] = (void*)+[](void*) {};
    android_syms["APerformanceHint_reportActualWorkDuration"] = (void*)+[](void*, long) {};

    // Register stubs for remaining android symbols (insert — won't overwrite
    // existing real hooks above).
    for (const char** p = android_symbols; *p != nullptr; p++) {
        android_syms.insert({*p, (void*)+[](void) -> int { return 0; }});
    }

    linker::load_library("libandroid.so", android_syms);

    // loadGameWindowLibrary registers callbacks into a linker symbol map.
    // It's called here (by side effect) via the extern "C" wrapper.
    // The implementation is in core_patches_stub.cpp (C++ lambdas).
    // Cannot be Rust because the lambdas capture C++ shared_ptrs.
    CorePatches::loadGameWindowLibrary();
}

/// Create the game window via GameWindowManager and set up GLES2 symbols.
/// Call this AFTER mc_setup_android_hooks but BEFORE mc_load_minecraft.
extern "C" void mc_create_window_and_setup_graphics() {
    // XInitThreads must be called before any X11 operations to enable
    // thread-safe X11. Mesa's EGL driver on X11 needs this for cross-thread
    // eglMakeCurrent to work.
    typedef int (*XInitThreadsFn)(void);
    XInitThreadsFn xinit = (XInitThreadsFn)dlsym(RTLD_DEFAULT, "XInitThreads");
    if (xinit) {
        xinit();
        Log::info("LAUNCHER", "XInitThreads() called successfully");
    } else {
        Log::warn("LAUNCHER", "XInitThreads not available");
    }

    Log::info("LAUNCHER", "Creating window via GameWindowManager...");

    // Create the game window (needed for eglGetProcAddress to return real GL symbols).
    // Use FakeLooper::setWindow so that when ALooper_prepare is called during
    // startGame, FakeLooper reuses this window instead of creating a second one.
    auto windowManager = GameWindowManager::getManager();
    Log::info("LAUNCHER", "GameWindowManager created, creating window...");

    // Use a large default size — the user can toggle fullscreen with FN11
    auto window = windowManager->createWindow("Minecraft", 1600, 1200, GraphicsApi::OPENGL_ES2);
    Log::info("LAUNCHER", "Window created successfully");
    FakeLooper::setWindow(window);

    // Set up FakeEGL — wraps the real proc addr resolver and registers
    // real EGL symbol implementations (eglMakeCurrent, eglSwapBuffers, etc.)
    // so the game's rendering thread can activate the GL context.
    auto procAddr = windowManager->getProcAddrFunc();
    FakeEGL::setProcAddrFunction(reinterpret_cast<void* (*)(const char*)>(procAddr));
    FakeEGL::installLibrary();

    // Install GL function overrides (MESA 23.1 blackscreen workarounds, NVIDIA stubs,
    // GLCorePatch for desktop GL compat). This is normally called from
    // FakeLooper::initializeWindow() when creating a new window, but when a
    // pendingWindow is adopted, initializeWindow() returns early without calling it.
    FakeEGL::setupGLOverrides();

    // Save the real EGL handles (display, surface, context) that eglut created,
    // so FakeEGL's eglMakeCurrent can call the real eglMakeCurrent directly
    // instead of going through the eglut backend (which fails on secondary threads).
    FakeEGL::saveCurrentWindowHandle();
    // Save the X11 native window handle so per-thread contexts can create window surfaces
    FakeEGL::saveNativeWindow(eglutGetWindowHandle());
    // Release the GL context from this thread so the game thread can bind it.
    // Without this, real_eglMakeCurrent fails with EGL_BAD_MATCH because the
    // context is still current on this thread.
    FakeEGL::releaseContext();
    Log::info("LAUNCHER", "FakeEGL installed");

    // Register GLES2 symbols using FakeEGL's proc address resolver (which
    // includes overrides for bug workarounds).
    // NOTE: This must use linker::relocate on the soinfo created in
    // mc_load_core_libraries, NOT a second load_library call (which would
    // create an orphaned duplicate soinfo whose symbols are never reached).
    mc_relocate_glesv2_symbols(fake_egl::eglGetProcAddress);
    Log::info("LAUNCHER", "Graphics setup complete");
}

extern "C" {

struct mc_jni_context {
    JniSupport* support;
};

mc_jni_context* mc_jni_create() {
    auto* ctx = new mc_jni_context();
    ctx->support = new JniSupport();
    // Set the JniSupport reference for FakeLooper so it can call onWindowCreated
    // when ALooper_prepare is invoked by the game during startGame.
    FakeLooper::setJniSupport(ctx->support);
    return ctx;
}

void mc_jni_destroy(mc_jni_context* ctx) {
    if (!ctx) return;
    delete ctx->support;
    delete ctx;
}

/// Loads libminecraftpe.so with the same SwappyGL hooks and mouse/fullscreen
/// callbacks the original C++ launcher uses. This is essential on modern
/// versions (1.21.30+) where the game uses Swappy for frame pacing and will
/// fail to present (black screen) if Swappy is left unstubbed.
void* mc_load_minecraft() {
    // Fill SwappyGL hooks from Rust
    std::vector<mcpelauncher_hook_t> hooks(15);
    fake_swappygl_fill_hooks(hooks.data(), hooks.size());

    void* handle = MinecraftUtils::loadMinecraftLib(
        reinterpret_cast<void*>(&core_patches_show_mouse_pointer),
        reinterpret_cast<void*>(&core_patches_hide_mouse_pointer),
        reinterpret_cast<void*>(&core_patches_set_fullscreen),
        reinterpret_cast<void*>(&FakeLooper::onGameActivityClose),
        hooks);
    if (handle) {
        core_patches_install(handle);
    }
    return handle;
}

// Game handle accessed via global for the capture-less lambda (function pointer).
static void* g_jni_game_handle = nullptr;

void mc_jni_register_natives(mc_jni_context* ctx, void* game_handle) {
    if (!ctx || !ctx->support) return;
    g_jni_game_handle = game_handle;
    ctx->support->registerMinecraftNatives(+[](const char* sym) -> void* {
        return linker::dlsym(g_jni_game_handle, sym);
    });
}

void mc_jni_start_game(mc_jni_context* ctx, void* game_handle) {
    if (!ctx || !ctx->support) return;
    g_jni_game_handle = game_handle;

    auto* gameOnCreate = (void (*)(GameActivity*, void*, size_t))
        linker::dlsym(game_handle, "GameActivity_onCreate");
    auto* stbiLoad = linker::dlsym(game_handle, "stbi_load_from_memory");
    auto* stbiFree = linker::dlsym(game_handle, "stbi_image_free");

    // Run startGame on the main thread (not a helper thread) because the
    // JNI env for this thread was already created during JniSupport
    // construction (VM::initialize calls CreateEnv which sets the thread-
    // local JniEnvContext::env.env).  The cmake-built libjnivm.a has
    // EnableJNIVMGC but our Rust build.rs does not — without that flag,
    // AttachCurrentThread skips per-thread env creation, so a helper
    // thread would have no env and throw "No Env in this thread".
    //
    // storeStartThreadId is called so that the game's
    // first pthread_create (from GameActivity_onCreate → android_main)
    // can be intercepted. In the Rust build, hookLibC is NOT called
    // (pthread_create creates real threads), but this call is kept
    // for compatibility.
    fake_thread_mover_store_start_thread_id();
    fprintf(stderr, "=== STARTING GAME ON MAIN THREAD ===\n");
    try {
        ctx->support->startGame(nullptr, gameOnCreate, stbiLoad, stbiFree);
    } catch (const std::exception& e) {
        Log::error("LAUNCHER", "startGame threw: %s", e.what());
        return;
    } catch (...) {
        Log::error("LAUNCHER", "startGame threw unknown exception");
        return;
    }

    // Main thread blocks forever (game thread runs independently)
    fprintf(stderr, "=== ENTERING GAME EVENT LOOP ===\n");
    fake_thread_mover_execute_main_thread();
    fprintf(stderr, "=== GAME EVENT LOOP EXITED ===\n");
    ctx->support->setLooperRunning(false);
}

/// C-linkage wrapper for fake_egl::eglSwapBuffers, called from Rust's
/// SwappyGL_swap stub.
extern "C" int mc_egl_swap_buffers(void* display, void* surface) {
    return fake_egl::eglSwapBuffers((EGLDisplay)display, (EGLSurface)surface);
}

} // extern "C"

/// C bridge: extern "C" entry points for Rust main.rs.
///
/// All linker, soinfo, __loader_*, and MinecraftUtils symbols come from their
/// respective cmake-built static libraries. This file provides only the thin
/// extern "C" bridge that Rust code calls.
///
/// NOTE: No mcpelauncher-linker/bionic headers are included — GCC 16.1.1
/// conflicts with libc-shim symbol overrides. Local mcpelauncher-client
/// headers (fake_assetmanager.h) are safe — they only use standard C++ types.

#include <cstddef>
#include <cstdint>
#include <cstring>
#include <string>
#include <unordered_map>
#include <vector>
#include <dlfcn.h>
#include <cstdio>

// Forward declarations for ThreadMover and CorePatches (avoid bionic header deps)
class ThreadMover {
public:
    static void hookLibC(std::unordered_map<std::string, void*>& syms);
};
class CorePatches {
public:
    static void install(void* handle);
};

// Include auto-generated stub symbols
#include <minecraft/imported/glesv2_symbols.h>

// ---------- Forward declarations ----------

struct MinecraftVersion {
    static void init(std::string, int);
};

// PathHelper pathInfo is defined in libmcpelauncher-common.a (BSS symbol)
// Use extern declaration to access it through the C++ mangled symbol.
// The struct layout must match PathHelper::PathInfo in the header.
struct PathHelper_Info {
    std::string appDir, homeDir, dataHome;
    std::vector<std::string> dataDirs;
    std::string cacheHome, overrideDataDir, overrideCacheDir, gameDir;
};

extern PathHelper_Info _ZN10PathHelper8pathInfoE;

static void pathhelper_setGameDir(const std::string& gameDir) {
    _ZN10PathHelper8pathInfoE.gameDir = gameDir;
    if (!_ZN10PathHelper8pathInfoE.gameDir.empty() && _ZN10PathHelper8pathInfoE.gameDir.back() != '/')
        _ZN10PathHelper8pathInfoE.gameDir += '/';
}
static void pathhelper_setDataDir(const std::string& dataDir) {
    _ZN10PathHelper8pathInfoE.overrideDataDir = dataDir;
    if (!_ZN10PathHelper8pathInfoE.overrideDataDir.empty() && _ZN10PathHelper8pathInfoE.overrideDataDir.back() != '/')
        _ZN10PathHelper8pathInfoE.overrideDataDir += '/';
}
static void pathhelper_setCacheDir(const std::string& cacheDir) {
    _ZN10PathHelper8pathInfoE.overrideCacheDir = cacheDir;
    if (!_ZN10PathHelper8pathInfoE.overrideCacheDir.empty() && _ZN10PathHelper8pathInfoE.overrideCacheDir.back() != '/')
        _ZN10PathHelper8pathInfoE.overrideCacheDir += '/';
}

struct MinecraftUtils {
    static std::unordered_map<std::string, void*> getLibCSymbols();
    static void* loadLibM();
    static void setupHybris();
    static void setupGLES2Symbols(void* (*resolver)(const char*));
    static const char* getLibraryAbi();
};

// libc-shim symbol struct
struct shim_shimmed_symbol {
    const char* name;
    void* value;
};

namespace linker {
    void init();
    void* load_library(const char*, const std::unordered_map<std::string, void*>&);
    void relocate(void* handle, const std::unordered_map<std::string, void*>& symbols);
}

// Forward declarations for bionic linker functions
extern "C" void __loader_android_update_LD_LIBRARY_PATH(const char*);
extern "C" void* __loader_android_dlopen_ext(const char*, int, const void*, const void*);
static void linker_update_LD_LIBRARY_PATH(const char* path) {
    __loader_android_update_LD_LIBRARY_PATH(path);
}

// Handle for libGLESv2.so soinfo, saved so that mc_relocate_glesv2_symbols
// can replace stub entries with real GL functions via linker::relocate.
static void* g_glesv2_handle = nullptr;

extern "C" {

void mc_setup_paths(const char* g, const char* d, const char* c) {
    if (g) pathhelper_setGameDir(std::string(g));
    if (d) pathhelper_setDataDir(std::string(d));
    if (c) pathhelper_setCacheDir(std::string(c));
}

void mc_init_version(const char* pkg, int code) {
    MinecraftVersion::init(std::string(pkg), code);
}

/// No forward declarations needed — all extern symbols resolve via static libraries.

/// Calls MinecraftUtils::getLibCSymbols() and copies merged C++ + Rust symbols
/// into the caller-supplied buffer. Returns the number of symbols written.
int mc_get_libc_symbols(shim_shimmed_symbol* buf, int max_entries) {
    auto syms = MinecraftUtils::getLibCSymbols();
    static std::vector<std::string> persistent;
    persistent.clear();
    persistent.reserve(static_cast<size_t>(max_entries));
    int count = 0;
    for (auto& [name, val] : syms) {
        if (count >= max_entries) break;
        persistent.push_back(name);
        buf[count].name = persistent.back().c_str();
        buf[count].value = val;
        count++;
    }
    return count;
}

extern "C" void linker_init_rust();

/// Runs the core init sequence that the original main.cpp performs.
/// Call this AFTER mc_setup_paths and mc_init_version.
int mc_load_core_libraries(const char* lib_dir) {
    // 0) Initialize Rust linker (primary). Also initializes C++ bionic linker
    //    state internally so game library loading via C++ linker still works.
    linker_init_rust();

    // 1) Register libc symbols with the C++ linker
    auto libC = MinecraftUtils::getLibCSymbols();
    // NOTE: ThreadMover::hookLibC is intentionally NOT called here.
    // The original C++ launcher runs startGame on a detached helper thread so
    // the main thread is free for executeMainThread. In the Rust bridge, both
    // startGame and executeMainThread run on the main thread. If we intercept
    // pthread_create, GameActivity_onCreate blocks waiting for the game thread
    // to signal readiness, but the thread never starts (stored in promise) → deadlock.
    // Without the hook, the game creates a real thread, GameActivity_onCreate
    // waits for it to signal readiness (which it does after ALooper_prepare),
    // then returns. The main thread blocks on executeMainThread but the game
    // thread runs the event loop and renders.
    linker::load_library("libc.so", libC);

    // 2) Load libm
    MinecraftUtils::loadLibM();

    // 3) Setup hybris (loads libz, hooks android log, sets up mod API)
    MinecraftUtils::setupHybris();

    // 4) Register stub libraries that libminecraftpe.so depends on
    linker::load_library("libOpenSLES.so", std::unordered_map<std::string, void*>());
    linker::load_library("libGLESv1_CM.so", std::unordered_map<std::string, void*>());
    linker::load_library("libstdc++.so", std::unordered_map<std::string, void*>());

    // Register libGLESv2.so with stub functions (real GL context needed for proper symbols)
    {
        std::unordered_map<std::string, void*> gl_syms;
        for (const char** p = glesv2_symbols; *p != nullptr; p++) {
            gl_syms[*p] = (void*)+[](void) -> int { return 0; };
        }
        g_glesv2_handle = linker::load_library("libGLESv2.so", gl_syms);
    }

    // EGL symbols are registered by FakeEGL::installLibrary() later, after window
    // creation.  BIND_NOW requires all symbols to be present before dlopen, so
    // FakeEGL::installLibrary() must be called BEFORE mc_load_minecraft.
    // NOTE: "libEGL.so" is deliberately NOT registered here — FakeEGL handles it.
    // NOTE: android hooks (libandroid.so) and game window library are set up in
    // mc_setup_android_hooks() — call it from Rust AFTER mc_load_core_libraries
    // but BEFORE mc_load_minecraft.
    linker::load_library("liblog.so", std::unordered_map<std::string, void*>());
    linker::load_library("libmcpelauncher_gamewindow.so", std::unordered_map<std::string, void*>());

    // 5) Set up library search path so dlopen_ext can find libminecraftpe.so etc.
    //    This must match the original main.cpp: update_LD_LIBRARY_PATH with the lib dir
    std::string libDir = _ZN10PathHelper8pathInfoE.gameDir + "lib/" + MinecraftUtils::getLibraryAbi();
    linker_update_LD_LIBRARY_PATH(libDir.c_str());

    return 0;
}

/// Registered GLES2 symbols using a real eglGetProcAddress-like resolver.
/// Call this AFTER mc_load_core_libraries but BEFORE mc_load_minecraft.
/// The resolver must be a function that takes a GL symbol name and returns
/// a function pointer from the real GL driver.
void mc_setup_graphics(void* (*proc_addr)(const char*)) {
    MinecraftUtils::setupGLES2Symbols(proc_addr);
}

/// Replace the stub libGLESv2.so symbols with real GL functions obtained via
/// the given resolver.  This uses linker::relocate() on the existing soinfo
/// rather than calling load_library a second time (which would create a
/// duplicate soinfo that symbol lookups never reach).
void mc_relocate_glesv2_symbols(void* (*resolver)(const char*)) {
    if (!g_glesv2_handle) {
        fprintf(stderr, "LAUNCHER: g_glesv2_handle is null — mc_load_core_libraries not called yet\n");
        return;
    }
    std::unordered_map<std::string, void*> syms;
    for (const char** p = glesv2_symbols; *p != nullptr; p++) {
        if (auto* fn = resolver(*p)) {
            syms[*p] = fn;
        }
    }
    linker::relocate(g_glesv2_handle, syms);
}

} // extern "C"

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
extern "C" void* __loader_dlopen(const char* filename, int flags, const void* caller_addr);
extern "C" void* __loader_dlsym(void* handle, const char* symbol, const void* caller_addr);
static void linker_update_LD_LIBRARY_PATH(const char* path) {
    __loader_android_update_LD_LIBRARY_PATH(path);
}

// Bionic RTLD_NOLOAD (same value as glibc). Do not use system RTLD_GLOBAL
// values — they differ from bionic and are not needed here.
#ifndef MCPE_RTLD_NOLOAD
#define MCPE_RTLD_NOLOAD 0x4
#endif

// --- Rust linker FFI bridge ---
// Functions for mirroring C++ linker state to the Rust linker.
extern "C" size_t linker_load_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);
extern "C" void linker_add_symbols_to_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);

// Android log hooks defined in hybris_android_log_hook.cpp; need their addresses
// to mirror to the Rust linker's global_symbols table.
extern "C" void __android_log_print();
extern "C" void __android_log_vprint();
extern "C" void __android_log_write();
extern "C" void __android_log_assert();

/// Helper: register a stub library with the Rust linker.
/// Converts a C++ unordered_map to parallel C arrays for Rust FFI.
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

/// Helper: add symbols to an already-registered Rust linker library.
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

// --- Rust linker extern "C" functions for Phase 2 ---
extern "C" void linker_rust_add_search_path(const char* path);
extern "C" size_t linker_rust_dlopen_ext(const char* filename, int flags,
                                         const char* const* hook_names, void* const* hook_vals,
                                         size_t hook_count);
extern "C" size_t linker_rust_find_library(const char* name);
extern "C" int linker_mirror_registration_enabled_rust();

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
extern "C" void mcpelauncher_linker_cpp_init();

/// Runs the core init sequence that the original main.cpp performs.
/// Call this AFTER mc_setup_paths and mc_init_version.
int mc_load_core_libraries(const char* lib_dir) {
    // 0) Initialize Rust linker first, then C++ bionic linker.
    //    Phase 2: Rust-primary stub registration. Rust owns all stub state;
    //    C++ soinfo is still created only for libc.so and libstdc++.so
    //    (needed by loadMinecraftLib's linker::dlopen calls). Other stubs
    //    (libOpenSLES, libGLESv1_CM, libGLESv2, liblog, libmcpelauncher_gamewindow)
    //    are Rust-only with C++ registration handled elsewhere.
    linker_init_rust();
    mcpelauncher_linker_cpp_init();

    // 1) Register libc symbols with Rust linker first, then C++
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
    rust_load_stub("libc.so", libC);
    // Phase 2: mirror libc into C++ bionic solist ONLY when mirrors are enabled
    // (i.e. not RUST_ONLY). Under RUST_ONLY the stub exists only in Rust state.
    if(linker_mirror_registration_enabled_rust()) {
        linker::load_library("libc.so", libC);
    }

    // 2) Load libm
    MinecraftUtils::loadLibM();

    // 3) Setup hybris (loads libz, hooks android log, sets up mod API)
    MinecraftUtils::setupHybris();

    // 4) Register stub libraries that libminecraftpe.so depends on
    //
    // libHttpClient.Android.so is stubbed before the game loads. Loading the
    // real ELF currently still SIGSEGVs during XAL init (bad object pointer
    // 0x20 after PlayGames), even with JUMP_SLOT self-resolution. Stubs keep
    // the game offline-capable; HCInitialize returns S_OK so XAL does not
    // throw Xal::Exception on hard-fail init.
    {
        extern void http_client_register_stubs();
        http_client_register_stubs();
    }
    {

        // libOpenSLES.so: Rust-only stub registration.
        // C++ registration happens in MinecraftUtils::setupHybris().
        auto empty = std::unordered_map<std::string, void*>();
        rust_load_stub("libOpenSLES.so", empty);
    }
    {
        // libGLESv1_CM.so: Rust-only stub registration.
        // C++ registration happens in MinecraftUtils::setupHybris().
        auto empty = std::unordered_map<std::string, void*>();
        rust_load_stub("libGLESv1_CM.so", empty);
    }
    {
        // libstdc++.so: C++ registration kept — loadMinecraftLib calls
        // linker::dlopen("libstdc++.so", 0) directly and expects a C++ handle.
        auto empty = std::unordered_map<std::string, void*>();
        rust_load_stub("libstdc++.so", empty);
        if(linker_mirror_registration_enabled_rust()) {
            linker::load_library("libstdc++.so", empty);
        }
    }

    // Register libGLESv2.so with stub functions (real GL context needed for proper symbols)
    // Phase 2: Rust-only — no C++ consumer needs the soinfo after GLES relocate
    // was ported to linker_add_symbols_to_library_rust.
    {
        std::unordered_map<std::string, void*> gl_syms;
        for (const char** p = glesv2_symbols; *p != nullptr; p++) {
            gl_syms[*p] = (void*)+[](void) -> int { return 0; };
        }
        rust_load_stub("libGLESv2.so", gl_syms);
    }

    // EGL symbols are registered by FakeEGL::installLibrary() later, after window
    // creation.  BIND_NOW requires all symbols to be present before dlopen, so
    // FakeEGL::installLibrary() must be called BEFORE mc_load_minecraft.
    // NOTE: "libEGL.so" is deliberately NOT registered here — FakeEGL handles it.
    // NOTE: android hooks (libandroid.so) and game window library are set up in
    // mc_setup_android_hooks() — call it from Rust AFTER mc_load_core_libraries
    // but BEFORE mc_load_minecraft.
    {
        std::unordered_map<std::string, void*> log_syms;
        log_syms["__android_log_print"] = (void*)__android_log_print;
        log_syms["__android_log_vprint"] = (void*)__android_log_vprint;
        log_syms["__android_log_write"] = (void*)__android_log_write;
        log_syms["__android_log_assert"] = (void*)__android_log_assert;
        rust_load_stub("liblog.so", log_syms);
    }
    {
        // libmcpelauncher_gamewindow.so: Rust-only stub registration.
        // Full C++ registration (with callbacks) happens in
        // CorePatches::loadGameWindowLibrary().
        auto empty = std::unordered_map<std::string, void*>();
        rust_load_stub("libmcpelauncher_gamewindow.so", empty);
    }

    // 5) Set up library search path so dlopen_ext can find libminecraftpe.so etc.
    //    This must match the original main.cpp: update_LD_LIBRARY_PATH with the lib dir
    std::string libDir = _ZN10PathHelper8pathInfoE.gameDir + "lib/" + MinecraftUtils::getLibraryAbi();
    linker_update_LD_LIBRARY_PATH(libDir.c_str());

    // Also register the search path for the Rust linker
    linker_rust_add_search_path(libDir.c_str());

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
/// the given resolver.  Phase 2: Rust-only — uses linker_add_symbols_to_library_rust
/// instead of linker::relocate. The game (Rust-loaded) binds real GL entry points
/// from the Rust linker's global_symbols during dlopen_ext relocation.
void mc_relocate_glesv2_symbols(void* (*resolver)(const char*)) {
    std::unordered_map<std::string, void*> syms;
    for (const char** p = glesv2_symbols; *p != nullptr; p++) {
        if (auto* fn = resolver(*p)) {
            syms[*p] = fn;
        }
    }
    if (syms.empty()) {
        fprintf(stderr, "LAUNCHER: no GLESv2 symbols resolved (missing GL driver?)\n");
        return;
    }
    rust_add_symbols("libGLESv2.so", syms);
    fprintf(stderr, "LAUNCHER: relocated %zu GLESv2 symbols into Rust linker\n",
            syms.size());
}

} // extern "C"

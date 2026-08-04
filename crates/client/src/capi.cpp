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

// PathHelper state is owned by Rust (crates/client/src/path_helper.rs); the
// C++ PathHelper class (path_helper.cpp) has been deleted. These are the Rust
// FFI entry points that replaced it.
extern "C" void path_helper_set_game_dir(const char* dir);
extern "C" void path_helper_set_data_dir(const char* dir);
extern "C" void path_helper_set_cache_dir(const char* dir);
extern "C" const char* path_helper_get_game_dir();

struct MinecraftUtils {
    static std::unordered_map<std::string, void*> getLibCSymbols();
    static void* loadLibM();
    static void setupHybris();
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

extern "C" {

void mc_setup_paths(const char* g, const char* d, const char* c) {
    if (g) path_helper_set_game_dir(g);
    if (d) path_helper_set_data_dir(d);
    if (c) path_helper_set_cache_dir(c);
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
    // 0) Initialize Rust linker (single owner of all libraries).
    //    Phase 2: Rust-primary stub registration. Rust owns all stub state;
    //    other stubs (libOpenSLES, libGLESv1_CM, libGLESv2, liblog,
    //    libmcpelauncher_gamewindow) are Rust-only.
    linker_init_rust();

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

    // 2) Load libm
    MinecraftUtils::loadLibM();

    // 3) Setup hybris (loads libz, hooks android log, sets up mod API)
    MinecraftUtils::setupHybris();

    // 4) Register stub libraries that libminecraftpe.so depends on
    //
    // libHttpClient.Android.so is NOT stubbed: the real ELF from the game dir
    // is loaded by the Rust linker (matching mcpelauncher-manifest). Its
    // HCHttpCall* API routes through the JNI com.xbox.httpclient classes,
    // whose natives the Rust/C++ http_client implementation provides. The
    // linker dlsym global-scope fallback makes the Java_com_xbox_httpclient_*
    // callback symbols from libHttpClient.Android.so resolvable.
    {

        // libOpenSLES.so: Rust-only stub registration.
        auto empty = std::unordered_map<std::string, void*>();
        rust_load_stub("libOpenSLES.so", empty);
    }
    {
        // libGLESv1_CM.so: Rust-only stub registration.
        auto empty = std::unordered_map<std::string, void*>();
        rust_load_stub("libGLESv1_CM.so", empty);
    }
    {
        // libstdc++.so: Rust-only stub registration.
        auto empty = std::unordered_map<std::string, void*>();
        rust_load_stub("libstdc++.so", empty);
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
    std::string libDir = std::string(path_helper_get_game_dir()) + "lib/" + MinecraftUtils::getLibraryAbi();
    linker_rust_add_search_path(libDir.c_str());

    return 0;
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

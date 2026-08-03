#pragma once

#include <unordered_map>
#include <mcpelauncher/linker.h>

// Phase 4 Rust FFI helpers (friend of MinecraftUtils — access preinitHooks).
// Declared here with extern "C" so the friend declarations below refer to the
// same (C-linkage) entities the Rust side links against.
extern "C" const char* mc_find_data_file(const char* path);
extern "C" size_t mc_get_preinit_hooks(const char** names, void** vals, size_t max);
extern "C" void mc_finalize_load(void* handle, const char* const* names, void* const* vals, size_t count);

class MinecraftUtils {
private:
    static void setupApi();

    struct HookEntry {
        void* value;
        void* user;
        void (*callback)(void*, void*);
    };

    static std::unordered_map<std::string, HookEntry> preinitHooks;

    friend const char* mc_find_data_file(const char*);
    friend size_t mc_get_preinit_hooks(const char**, void**, size_t);
    friend void mc_finalize_load(void*, const char* const*, void* const*, size_t);

public:
    static std::unordered_map<std::string, void*> getApi();

    static void workaroundLocaleBug();

    static std::unordered_map<std::string, void*> getLibCSymbols();
    static void* loadLibM();

    static void setupHybris();

    static void* loadFMod();
    static void stubFMod();

    static const char* getLibraryAbi();

    static size_t getLibraryBase(void* handle);
};

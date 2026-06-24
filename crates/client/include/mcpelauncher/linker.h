#pragma once

// Minimal forward declarations of linker functions used by mcpelauncher-client code.
// Replaces mcpelauncher-linker/public_include/mcpelauncher/linker.h which pulls in bionic
// headers that are incompatible with GCC 16.1.1.

#include <cstddef>
#include <string>
#include <unordered_map>

// Satisfy the android_dlextinfo* parameter in linker::dlopen_ext (declared but never called
// by the jni support code; we only provide the type to keep the declaration valid).
struct android_dlextinfo;

extern "C" {
    void* __loader_dlopen(const char* filename, int flags, const void* caller_addr);
    void* __loader_dlsym(void* handle, const char* symbol, const void* caller_addr);
    int __loader_dladdr(const void* addr, void* info);
    int __loader_dlclose(void* handle);
    char* __loader_dlerror();
    int __loader_dl_iterate_phdr(int (*cb)(void* info, size_t size, void* data), void* data);
    void __loader_android_update_LD_LIBRARY_PATH(const char* ld_library_path);
    void* __loader_android_dlopen_ext(const char* filename,
                           int flags,
                           const android_dlextinfo* extinfo,
                           const void* caller_addr);
}

// Hook descriptor used by MinecraftUtils::loadMinecraftLib. Defined here so
// mcpelauncher-client code (fake_swappygl, jni_bridge) can build hooks without
// pulling in the bionic android/dlext.h.
struct mcpelauncher_hook_t {
    const char* name;
    void* value;
};

namespace linker {

    inline void* dlopen(const char* filename, int flags) {
        return __loader_dlopen(filename, flags, nullptr);
    }

    inline void* dlopen_ext(const char* filename, int flags, const android_dlextinfo* extinfo) {
        return __loader_android_dlopen_ext(filename, flags, extinfo, nullptr);
    }

    inline void* dlsym(void* handle, const char* symbol) {
        return __loader_dlsym(handle, symbol, nullptr);
    }

    inline int dladdr(const void* addr, void* info) {
        return __loader_dladdr(addr, info);
    }

    inline int dlclose(void* handle) {
        return __loader_dlclose(handle);
    }

    inline char* dlerror() {
        return __loader_dlerror();
    }

    inline int dl_iterate_phdr(int (*cb)(void* info, size_t size, void* data), void* data) {
        return __loader_dl_iterate_phdr(cb, data);
    }

    inline void update_LD_LIBRARY_PATH(const char* ld_library_path) {
        __loader_android_update_LD_LIBRARY_PATH(ld_library_path);
    }

    void init();
    void* load_library(const char* name, const std::unordered_map<std::string, void*>& symbols);
    int unload_library(void*);
    void relocate(void* handle, const std::unordered_map<std::string, void*>& symbols);
    size_t get_library_base(void* handle);
    void get_library_code_region(void* handle, size_t& base, size_t& size);
    int dlclose_unlocked(void* handle);

}

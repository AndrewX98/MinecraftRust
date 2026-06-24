#pragma once

// Minimal forward declarations of linker functions used by jni_support.cpp.
// Avoids pulling in bionic <mcpelauncher/linker.h> which conflicts with GCC 16.1.1.

#include <cstddef>
#include <cstring>

namespace linker {

// From mcpelauncher-linker
extern "C" void* __loader_dlopen(const char* filename, int flags, void* caller_addr);
extern "C" void* __loader_dlsym(void* handle, const char* symbol, void* caller_addr);
extern "C" char* __loader_dlerror(void* caller_addr);
extern "C" int __loader_dlclose(void* handle);

inline void* dlopen(const char* filename, int flags, void* caller_addr = nullptr) {
    return __loader_dlopen(filename, flags, caller_addr);
}

inline void* dlsym(void* handle, const char* symbol) {
    return __loader_dlsym(handle, symbol, __builtin_return_address(0));
}

inline char* dlerror() {
    return __loader_dlerror(__builtin_return_address(0));
}

inline int dlclose_unlocked(void* handle) {
    return __loader_dlclose(handle);
}

}  // namespace linker

#include <mcpelauncher/hybris_utils.h>
#include <mcpelauncher/path_helper.h>
#include <log.h>
#include <dlfcn.h>

const char* HybrisUtils::TAG = "LinkerUtils";

extern "C" size_t linker_load_library_rust(const char*, const char* const*, void* const*, size_t);

void* HybrisUtils::loadLibraryOS(const char *name, std::string const &path, const char** symbols) {
    return loadLibraryOS(name, path, symbols, std::unordered_map<std::string, void*>());
}

void* HybrisUtils::loadLibraryOS(const char *name, std::string const &path, const char** symbols, std::unordered_map<std::string, void*> syms) {
    void* handle = dlopen(path.c_str(), RTLD_LAZY);
    if (handle == nullptr) {
        Log::error(TAG, "Failed to load OS library %s", path.c_str());
        return nullptr;
    }
    Log::trace(TAG, "Loaded OS library %s", path.c_str());
    int i = 0;
    while (true) {
        const char* sym = symbols[i];
        if (sym == nullptr)
            break;
        void* ptr = dlsym(handle, sym);
        if (ptr)
            syms[sym] = ptr;
        i++;
    }
    // Register the resolved OS-library symbols with the Rust linker's
    // global_symbols so Rust-loaded images (e.g. libfmod) resolve libm / etc.
    // imports from the Rust linker state instead of a C++ dlsym fallback.
    if (!syms.empty()) {
        std::vector<const char*> keys;
        std::vector<void*> vals;
        keys.reserve(syms.size());
        vals.reserve(syms.size());
        for (auto& [k, v] : syms) {
            keys.push_back(k.c_str());
            vals.push_back(v);
        }
        linker_load_library_rust(name, keys.data(), vals.data(), syms.size());
    }
    return handle;
}

void HybrisUtils::stubSymbols(const char *name, const char** symbols, void* stubfunc) {
    int i = 0;
    std::unordered_map<std::string, void*> syms;
    while (true) {
        const char* sym = symbols[i];
        if (sym == nullptr)
            break;
        syms[sym] = stubfunc;
        i++;
    }
    std::vector<const char*> keys;
    std::vector<void*> vals;
    keys.reserve(syms.size());
    vals.reserve(syms.size());
    for (auto& [k, v] : syms) {
        keys.push_back(k.c_str());
        vals.push_back(v);
    }
    linker_load_library_rust(name, keys.data(), vals.data(), keys.size());
}
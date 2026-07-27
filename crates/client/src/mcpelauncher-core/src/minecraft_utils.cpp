#include <mcpelauncher/minecraft_utils.h>
#include <mcpelauncher/patch_utils.h>
#include <mcpelauncher/hybris_utils.h>
#include <mcpelauncher/fmod_utils.h>
#include <mcpelauncher/hook.h>
#include <mcpelauncher/path_helper.h>
#include <mcpelauncher/minecraft_version.h>
#include <minecraft/imported/android_symbols.h>
#include <minecraft/imported/egl_symbols.h>
#include <minecraft/imported/libm_symbols.h>
#include <minecraft/imported/fmod_symbols.h>
#include <minecraft/imported/glesv2_symbols.h>
#include <minecraft/imported/libz_symbols.h>
#include <log.h>
#include <FileUtil.h>
#include <memory>
#include <mcpelauncher/linker.h>
#include <libc_shim.h>
#include <stdexcept>
#include <cstring>
#include <cstdlib>

// Rust linker extern "C" functions for Phase 2 diagnostic trial load
extern "C" size_t linker_rust_dlopen_ext(const char* filename, int flags,
                                         const char* const* hook_names,
                                         void* const* hook_vals, size_t hook_count);
extern "C" void* linker_rust_dlsym(size_t handle, const char* symbol);
extern "C" void linker_rust_add_search_path(const char* path);
extern "C" size_t linker_rust_find_library(const char* name);
extern "C" size_t linker_rust_dlopen_sqlite(const char* filename);
extern "C" size_t linker_load_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);
extern "C" void linker_add_symbols_to_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);

// Handle-type-agnostic dispatch wrappers (defined in mcpelauncher-linker/src/linker.cpp)
extern "C" void* mcpelauncher_dispatch_dlsym(void* handle, const char* name);
extern "C" size_t mcpelauncher_dispatch_get_library_base(void* handle);
#if defined(__APPLE__) && defined(__aarch64__)
#include <libkern/OSCacheControl.h>
#include <pthread.h>
#endif
#include <unistd.h>
#include <sys/wait.h>
#include <sys/stat.h>
#include <stdexcept>
#include <cstring>
#include <errno.h>
#include <EnvPathUtil.h>
#include <jnivm.h>

void MinecraftUtils::workaroundLocaleBug() {
    setenv("LC_ALL", "C", 1);  // HACK: Force set locale to one recognized by MCPE so that the outdated C++ standard library MCPE uses doesn't fail to find one
}

static bool ReadEnvFlag(const char* name, bool def = false) {
    auto val = getenv(name);
    if(!val) {
        return def;
    }
    std::string sval = val;
    return sval == "true" || sval == "1" || sval == "on";
}

extern "C" {
    size_t get_shimmed_symbols_len();
    void get_shimmed_symbols_fill(shim::shimmed_symbol* buf);
}

std::unordered_map<std::string, void*> MinecraftUtils::getLibCSymbols() {
    std::unordered_map<std::string, void*> syms;
    auto rustLen = get_shimmed_symbols_len();
    if(rustLen > 0) {
        auto buf = std::make_unique<shim::shimmed_symbol[]>(rustLen);
        get_shimmed_symbols_fill(buf.get());
        for(size_t i = 0; i < rustLen; i++) {
            if(!buf[i].value) {
                continue; // skip null — was C++ shim, now Rust-only
            }
            syms[buf[i].name] = buf[i].value;
        }
    }
    for(auto&& s : syms) {
        if(!s.second) {
            Log::error("RUST_SHIM", "Merged symbol %s is NULL!", s.first.c_str());
        }
    }
    return syms;
}

void* MinecraftUtils::loadLibM() {
#ifdef __APPLE__
    void* libmLib = HybrisUtils::loadLibraryOS("libm.so", "libm.dylib", libm_symbols, std::unordered_map<std::string, void*>{{std::string("sincos"), (void*)__sincos}, {std::string("sincosf"), (void*)__sincosf}});
#elif defined(__FreeBSD__)
    void* libmLib = HybrisUtils::loadLibraryOS("libm.so", "libm.so", libm_symbols);
#else
    void* libmLib = HybrisUtils::loadLibraryOS("libm.so", "libm.so.6", libm_symbols);
#endif
    if(libmLib == nullptr)
        throw std::runtime_error("Failed to load libm");
    return libmLib;
}

void* MinecraftUtils::loadFMod() {
    void* fmodLib = HybrisUtils::loadLibraryOS("libfmod.so", PathHelper::findDataFile(std::string("lib/native/") + getLibraryAbi() +
#ifdef __APPLE__
#if defined(__i386__)
                                                                                      // Minecraft releases linked against libc++-shared have to use a newer version of libfmod
                                                                                      // Throwing here allows using pulseaudio if available / starting the game without sound
                                                                                      (linker::dlopen("libc++_shared.so", 0) ? throw std::runtime_error("Fmod removed i386 support, after deprecation by Apple") : "/libfmod.dylib")
#else
                                                                                      "/libfmod.dylib"
#endif
#else
#ifdef __LP64__
                                                                                      "/libfmod.so.12.0"
#else
                                                                                      // Minecraft releases linked against libc++-shared have to use a newer version of libfmod
                                                                                      (linker::dlopen("libc++_shared.so", 0) ? "/libfmod.so.12.0" : "/libfmod.so.10.20")
#endif
#endif
                                                                                          ),
                                               fmod_symbols);
    if(fmodLib == nullptr)
        throw std::runtime_error("Failed to load fmod");
    return fmodLib;
}

void MinecraftUtils::stubFMod() {
    HybrisUtils::stubSymbols("libfmod.so", fmod_symbols, (void*)(void* (*)())[]() {
        Log::warn("Launcher", "FMod stub called");
        return (void*) nullptr; });
}

void MinecraftUtils::setupHybris() {
    HybrisUtils::loadLibraryOS("libz.so",
#ifdef __APPLE__
                               "libz.dylib"
#elif defined(__FreeBSD__)
                               "libz.so"
#else
                               "libz.so.1"
#endif
                               ,
                               libz_symbols);
    HybrisUtils::hookAndroidLog();
    setupApi();
    linker::load_library("libOpenSLES.so", {});
    linker::load_library("libGLESv1_CM.so", {});

    linker::load_library("libstdc++.so", {});
    linker::load_library("libz.so", {});  // needed for <0.17
}

static std::vector<const char*> convertToC(std::vector<std::string> const& v) {
    std::vector<const char*> ret;
    for (auto const& i : v)
        ret.push_back(i.c_str());
    ret.push_back(nullptr);
    return std::move(ret);
}


struct GoogleCredentials {
    const char* email;
    const char* token;
};

static std::string getUiExecutablePath() {
    std::string path;
#ifndef MCPELAUNCHER_UI_PATH
#define MCPELAUNCHER_UI_PATH "."
#endif
    if(EnvPathUtil::findInPath("mcpelauncher-ui-qt", path, MCPELAUNCHER_UI_PATH, EnvPathUtil::getAppDir().c_str()))
        return path;
    if(EnvPathUtil::findInPath("mcpelauncher-ui-qt", path))
        return path;
    return "mcpelauncher-ui-qt";
}

static void requestGoogleCredentials(void (*onsuccess)(GoogleCredentials creds), void (*onfailure)(const char* error)) {
    const void* caller_addr = __builtin_return_address(0);
    Dl_info info;
    if(linker::dladdr(caller_addr, &info) != 0) {
        Log::info("Launcher", "Google credentials requested from %s", info.dli_fname);

        std::vector<std::string> args = {getUiExecutablePath(), "--request-google-credentials", "-v" , "--mod", info.dli_fname};
        Log::info("Launcher", "Executing google credentials helper: %s", args[0].c_str());
        char ret[1024];

        int pipes[3][2];
        static const int PIPE_STDOUT = 0;
        static const int PIPE_STDERR = 1;
        static const int PIPE_STDIN = 2;
        static const int PIPE_READ = 0;
        static const int PIPE_WRITE = 1;

        pipe(pipes[PIPE_STDOUT]);
        pipe(pipes[PIPE_STDERR]);
        pipe(pipes[PIPE_STDIN]);

        int pid;
        if (!(pid = fork())) {
            signal(SIGPIPE, SIG_IGN);
            auto argvc = convertToC(args);
            dup2(pipes[PIPE_STDOUT][PIPE_WRITE], STDOUT_FILENO);
            dup2(pipes[PIPE_STDERR][PIPE_WRITE], STDERR_FILENO);
            dup2(pipes[PIPE_STDIN][PIPE_READ], STDIN_FILENO);
            close(pipes[PIPE_STDIN][PIPE_WRITE]);
            close(pipes[PIPE_STDOUT][PIPE_WRITE]);
            close(pipes[PIPE_STDERR][PIPE_WRITE]);
            close(pipes[PIPE_STDIN][PIPE_READ]);
            close(pipes[PIPE_STDOUT][PIPE_READ]);
            close(pipes[PIPE_STDERR][PIPE_READ]);
            int r = execvp(argvc[0], (char**) argvc.data());
            printf("Show: execvp() error %i %s", r, strerror(errno));
            close(STDOUT_FILENO);
            close(STDERR_FILENO);
            close(STDIN_FILENO);
            _exit(r);
        } else {
            close(pipes[PIPE_STDIN][PIPE_WRITE]);
            close(pipes[PIPE_STDIN][PIPE_READ]);

            close(pipes[PIPE_STDOUT][PIPE_WRITE]);
            close(pipes[PIPE_STDERR][PIPE_WRITE]);

            std::string outputStdOut;
            std::string outputStdErr;
            ssize_t r;
            while ((r = read(pipes[PIPE_STDOUT][PIPE_READ], ret, 1024)) > 0)
                outputStdOut += std::string(ret, (size_t) r);
            while ((r = read(pipes[PIPE_STDERR][PIPE_READ], ret, 1024)) > 0)
                outputStdErr += std::string(ret, (size_t) r);

            close(pipes[PIPE_STDOUT][PIPE_READ]);
            close(pipes[PIPE_STDERR][PIPE_READ]);

            int status;
            while(true) {
                int err = waitpid(pid, &status, 0);
                if(err == -1) {
                    if(errno == EINTR) {
                        continue;
                    }
                    onfailure(("Failed to wait for Google credentials process: " + std::string(strerror(errno))).data());
                    return;
                }
                if(WIFSIGNALED(status)) {
                    onfailure(("Google credentials process terminated by signal " + std::to_string(WTERMSIG(status))).data());
                    return;
                }
                if(!WIFEXITED(status)) {
                    onfailure("Google credentials process did not exit normally");
                    return;
                }
                break;
            }

            status = WEXITSTATUS(status);

            if (status == 0) {
                Log::info("Launcher", "Obtained Google credentials from helper"); 
                size_t creds = outputStdErr.find("CRED=");
                if(creds != std::string::npos) {
                    std::string credstr = outputStdErr.substr(creds + 5);
                    size_t newline = credstr.find('\n');
                    if(newline != std::string::npos) {
                        credstr = credstr.substr(0, newline);
                    }
                    size_t sep = credstr.find(':');
                    if(sep != std::string::npos) {
                        std::string email = credstr.substr(0, sep);
                        std::string token = credstr.substr(sep + 1);
                        onsuccess({email.c_str(), token.c_str()});
                        return;
                    }
                }

                //onsuccess({"user@example.com", "token123"});
                onfailure(("Failed to parse Google credentials from helper output" + std::to_string(status) + " stdout: " + outputStdOut + " stderr: " + outputStdErr).data());
            } else {
                onfailure(("Failed to get Google credentials exit code " + std::to_string(status) + " stdout: " + outputStdOut + " stderr: " + outputStdErr).data());
            }
        }
    } else {
        Log::error("Launcher", "Google credentials requested from unknown caller");
        onfailure("Unknown caller");
    }
}
template<bool isStatic, bool isGetter, bool isSetter, typename T> struct ModHandle;
template<typename T> struct ModHandle<true, false, false, T> : public jnivm::MethodHandle {
public:
    ModHandle(jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values)) : method(method) {}
    jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values);
    virtual T StaticInvoke(jnivm::ENV * env, jnivm::Class* clazz, const jvalue* values, jnivm::impl::MethodHandleBase<T>) override {
        jvalue ret = method(env->GetJNIEnv(), (jclass)(clazz), (jvalue*)values);
        return (T&)ret;
    }
};

template<> struct ModHandle<true, false, false, void> : public jnivm::MethodHandle {
public:
    ModHandle(jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values)) : method(method) {}
    jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values);
    virtual void StaticInvoke(jnivm::ENV * env, jnivm::Class* clazz, const jvalue* values, jnivm::impl::MethodHandleBase<void>) override {
        method(env->GetJNIEnv(), (jclass)(clazz), (jvalue*)values);
    }
};

template<typename T> struct ModHandle<false, false, false, T> : public jnivm::MethodHandle {
public:
    ModHandle(jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values)) : method(method) {}
    jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values);
    virtual T InstanceInvoke(jnivm::ENV * env, jobject obj, const jvalue* values, jnivm::impl::MethodHandleBase<T>) override {
        jvalue ret = method(env->GetJNIEnv(), obj, (jvalue*)values);
        return (T&)ret;
    }
};


template<> struct ModHandle<false, false, false, void> : public jnivm::MethodHandle {
public:
    ModHandle(jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values)) : method(method) {}
    jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values);
    virtual void InstanceInvoke(jnivm::ENV * env, jobject obj, const jvalue* values, jnivm::impl::MethodHandleBase<void>) override {
        method(env->GetJNIEnv(), obj, (jvalue*)values);
    }
};

template<typename T> struct ModHandle<true, true, false, T> : public jnivm::MethodHandle {
public:
    ModHandle(jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values)) : method(method) {}
    jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values);

    virtual T StaticGet(jnivm::ENV * env, jnivm::Class* clazz, const jvalue* values, jnivm::impl::MethodHandleBase<T>) {
        jvalue ret = method(env->GetJNIEnv(), (jclass)(clazz), (jvalue*)values);
        return (T&)ret;
    }
};

template<typename T> struct ModHandle<false, true, false, T> : public jnivm::MethodHandle {
public:
    ModHandle(jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values)) : method(method) {}
    jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values);

    virtual T InstanceGet(jnivm::ENV * env, jobject obj, const jvalue* values, jnivm::impl::MethodHandleBase<T>) {
        jvalue ret = method(env->GetJNIEnv(), obj, (jvalue*)values);
        return (T&)ret;
    }
};

template<typename T> struct ModHandle<true, false, true, T> : public jnivm::MethodHandle {
public:
    ModHandle(jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values)) : method(method) {}
    jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values);

    virtual void StaticSet(jnivm::ENV * env, jnivm::Class* clazz, const jvalue* values, jnivm::impl::MethodHandleBase<T>) {
        method(env->GetJNIEnv(), (jclass)(clazz), (jvalue*)values);
    }
};

template<typename T> struct ModHandle<false, false, true, T> : public jnivm::MethodHandle {
public:
    ModHandle(jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values)) : method(method) {}
    jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values);

    virtual void InstanceSet(jnivm::ENV * env, jobject obj, const jvalue* values, jnivm::impl::MethodHandleBase<T>) {
        method(env->GetJNIEnv(), obj, (jvalue*)values);
    }
};

template<bool isStatic, bool isGetter, bool isSetter>
bool createModHandleNoVoid(const std::shared_ptr<jnivm::Method>& method, char typeId, jvalue (*cbk)(JNIEnv* env, jobject thiz, jvalue* values)) {
    switch(typeId) {
        case 'Z':
            method->nativehandle = std::make_shared<ModHandle<isStatic, isGetter, isSetter, jboolean>>(cbk);
            break;
        case 'B':
            method->nativehandle = std::make_shared<ModHandle<isStatic, isGetter, isSetter, jbyte>>(cbk);
            break;
        case 'C':
            method->nativehandle = std::make_shared<ModHandle<isStatic, isGetter, isSetter, jchar>>(cbk);
            break;
        case 'S':
            method->nativehandle = std::make_shared<ModHandle<isStatic, isGetter, isSetter, jshort>>(cbk);
            break;
        case 'I':
            method->nativehandle = std::make_shared<ModHandle<isStatic, isGetter, isSetter, jint>>(cbk);
            break;
        case 'J':
            method->nativehandle = std::make_shared<ModHandle<isStatic, isGetter, isSetter, jlong>>(cbk);
            break;
        case 'F':
            method->nativehandle = std::make_shared<ModHandle<isStatic, isGetter, isSetter, jfloat>>(cbk);
            break;
        case 'D':   
            method->nativehandle = std::make_shared<ModHandle<isStatic, isGetter, isSetter, jdouble>>(cbk);
            break;
        case 'L':
        case '[':
            method->nativehandle = std::make_shared<ModHandle<isStatic, isGetter, isSetter, jobject>>(cbk);
            break;
        default:
            return false;
    }
    return true;
}

template<bool isStatic>
bool createModFunction(const std::shared_ptr<jnivm::Method>& method, const char *signature, const char *end, jvalue (*cbk)(JNIEnv* env, jobject thiz, jvalue* values)) {
    auto retType = std::find(signature, end, ')');
    if(retType == end) {
        return false;
    }
    switch(*(retType + 1)) {
        case 'V':
            method->nativehandle = std::make_shared<ModHandle<isStatic, false, false, void>>(cbk);
            break;
        default:
            return createModHandleNoVoid<isStatic, false, false>(method, *(retType + 1), cbk);
    }
    return true;
}

std::unordered_map<std::string, void*> MinecraftUtils::getApi() {
    std::unordered_map<std::string, void*> syms;
    // Deprecated use android liblog
#if !(defined(__APPLE__) && defined(__aarch64__))
    syms["mcpelauncher_log"] = (void*)Log::log;
    syms["mcpelauncher_vlog"] = (void*)Log::vlog;
#endif

    syms["mcpelauncher_preinithook2"] = (void*)(void (*)(const char*, void*, void*, void (*)(void*, void*)))[](const char* name, void* sym, void* user, void (*callback)(void*, void*)) {
        preinitHooks[name] = {sym, user, callback};
    };
    syms["mcpelauncher_preinithook"] = (void*)(void (*)(const char*, void*, void**))[](const char* name, void* sym, void** orig) {
        auto&& def = [](void* user, void* orig) {
            *(void**)user = orig;
        };
        preinitHooks[name] = {sym, orig, orig ? def : nullptr};
    };

    syms["mcpelauncher_hook"] = (void*)(void* (*)(void*, void*, void**))[](void* sym, void* hook, void** orig) {
        Dl_info i;
        if(!linker::dladdr(sym, &i)) {
            Log::error("Hook", "Failed to resolve hook for symbol %lx", (long unsigned)sym);
            return (void*)nullptr;
        }
        void* handle = linker::dlopen(i.dli_fname, 0);
        std::string tName = HookManager::translateConstructorName(i.dli_sname);
        auto ret = HookManager::instance.createHook(handle, tName.empty() ? i.dli_sname : tName.c_str(), hook, orig);
        linker::dlclose(handle);
        HookManager::instance.applyHooks();
        return (void*)ret;
    };

    syms["mcpelauncher_hook2"] = (void*)(void* (*)(void*, const char*, void*, void**))
        [](void* lib, const char* sym, void* hook, void** orig) {
        return (void*)HookManager::instance.createHook(lib, sym, hook, orig);
    };
    syms["mcpelauncher_hook2_add_library"] = (void*)(void (*)(void*))[](void* lib) {
        HookManager::instance.addLibrary(lib);
    };
    syms["mcpelauncher_hook2_remove_library"] = (void*)(void (*)(void*))[](void* lib) {
        HookManager::instance.removeLibrary(lib);
    };
    syms["mcpelauncher_hook2_delete"] = (void*)(void (*)(void*))[](void* hook) {
        HookManager::instance.deleteHook((HookManager::HookInstance*)hook);
    };
    syms["mcpelauncher_hook2_apply"] = (void*)(void (*)())[]() {
        HookManager::instance.applyHooks();
    };
#if defined(__APPLE__) && defined(__aarch64__)
    syms["mcpelauncher_patch"] = (void*)+[](void* address, void* data, size_t size) -> void* {
        pthread_jit_write_protect_np(0);
        auto ret = memcpy(address, data, size);
        sys_icache_invalidate(address, size);
        pthread_jit_write_protect_np(1);
        return ret;
    };
#else
    syms["mcpelauncher_patch"] = (void*)+[](void* address, void* data, size_t size) -> void* {
        return memcpy(address, data, size);
    };
#endif
    syms["mcpelauncher_host_dlopen"] = (void*)dlopen;
    syms["mcpelauncher_host_dlsym"] = (void*)dlsym;
    syms["mcpelauncher_host_dlclose"] = (void*)dlclose;
    syms["mcpelauncher_relocate"] = (void*)+[](void* handle, const char* name, void* hook) {
        linker::relocate(handle, {{name, hook}});
    };
    struct hook_entry {
        const char* name;
        void* hook;
    };
    syms["mcpelauncher_relocate2"] = (void*)+[](void* handle, size_t count, hook_entry* entries) {
        std::unordered_map<std::string, void*> ventries;
        for(size_t i = 0; i < count; i++) {
            ventries[entries[i].name] = entries[i].hook;
        }
        linker::relocate(handle, ventries);
    };
    syms["mcpelauncher_load_library"] = (void*)+[](const char* name, size_t count, hook_entry* entries) {
        std::unordered_map<std::string, void*> ventries;
        for(size_t i = 0; i < count; i++) {
            ventries[entries[i].name] = entries[i].hook;
        }
        linker::load_library(name, ventries);
    };
    syms["mcpelauncher_unload_library"] = (void*)linker::unload_library;
    syms["mcpelauncher_dlclose_unlocked"] = (void*)linker::dlclose_unlocked;
    syms["mcpelauncher_package_name"] = (void*)MinecraftVersion::package.c_str();
    syms["mcpelauncher_package_version_code"] = (void*)&MinecraftVersion::code;
    syms["mcpelauncher_package_version_major"] = (void*)&MinecraftVersion::major;
    syms["mcpelauncher_package_version_minor"] = (void*)&MinecraftVersion::minor;
    syms["mcpelauncher_package_version_patch"] = (void*)&MinecraftVersion::patch;
    syms["mcpelauncher_package_version_revision"] = (void*)&MinecraftVersion::revision;

    syms["mcpelauncher_request_google_credentials"] = (void*)requestGoogleCredentials;

    // jnivm api to provide java class implementations from mods

    // - jnivm::Object should be able to hold fields values additionally to methods
    // - jnivm::Class should be able to hold static field values
    // - api to get the native method pointer by name and signature e.g. VMRunner.executeVM that has no public symbol name
    // - (to call others use jni api)
    // - api to register (static)method/(static)field getter(setter)/constructor
    // - `void jnivm_register_method(JNIEnv* env, jclass cl, int type, const char * signature, const char * name, jvalue (*method)(JNIEnv* env, jobject thiz, jvalue* values))`
    //     - type & 1 => static (if true thiz is a jclass instance)
    //     - type & 2 => method
    //     - type & 4 => getter
    //     - type & 8 => setter

    syms["jnivm_register_method"] = (void*)+[](JNIEnv* env, jclass cl, int type, const char* name, const char* signature, jvalue (*cbk)(JNIEnv* env, jobject thiz, jvalue* values)) -> bool {
        auto c = (jnivm::Class*)cl;
        auto isStatic = (type & 1) != 0;
        std::shared_ptr<jnivm::Method> method;
        if(type & 2) { // method
            auto ccl = std::find_if(c->methods.begin(), c->methods.end(),
                                    [name, signature, isStatic](std::shared_ptr<jnivm::Method> &m) {
                                        return m->_static == isStatic && m->name == name && m->signature == signature;
                                    });
            if (ccl != c->methods.end()) {
                method = *ccl;
            } else {
                method = std::make_shared<jnivm::Method>();
                method->name = name;
                method->_static = isStatic;
                method->signature = signature;
                c->methods.push_back(method);
            }
        }

        auto org = signature;
        const char* end = signature + strlen(signature);
        if(type & 1) { // static
            if(type & 2) { // method
                return createModFunction<true>(method, org, end, cbk);
            }
            if(type & 4) { // getter
                return createModHandleNoVoid<true, true, false>(method, *org, cbk);
            }
            if(type & 8) { // setter
                return createModHandleNoVoid<true, false, true>(method, *org, cbk);
            }
        } else {
            // Instance
            if(type & 2) { // method
                return createModFunction<false>(method, org, end, cbk);
            }
            if(type & 4) { // getter
                return createModHandleNoVoid<false, true, false>(method, *org, cbk);
            }
            if(type & 8) { // setter
                return createModHandleNoVoid<false, false, true>(method, *org, cbk);
            }
        }
        return false;
    };

    return syms;
}

void MinecraftUtils::setupApi() {
    linker::load_library("libmcpelauncher_mod.so", getApi());
}

std::unordered_map<std::string, MinecraftUtils::HookEntry> MinecraftUtils::preinitHooks;

void* MinecraftUtils::loadMinecraftLib(void* showMousePointerCallback, void* hideMousePointerCallback, void* fullscreenCallback, void* closeCallback, std::vector<mcpelauncher_hook_t> hooks) {
    auto libcxx = linker::dlopen("libc++_shared.so", 0);
    if(!libcxx) {
        Log::error("MinecraftUtils", "Failed to load libc++_shared: %s", linker::dlerror());
    }
    // loading libfmod standalone depends on these symbols, libminecraftpe.so changes the loading automatically
    auto libstdcxx = linker::dlopen("libstdc++.so", 0);
    if(libcxx) {
        auto __cxa_pure_virtual = linker::dlsym(libcxx, "__cxa_pure_virtual");
        auto __cxa_guard_acquire = linker::dlsym(libcxx, "__cxa_guard_acquire");
        auto __cxa_guard_release = linker::dlsym(libcxx, "__cxa_guard_release");

        if(__cxa_pure_virtual && __cxa_guard_acquire && __cxa_guard_release) {
            linker::relocate(libstdcxx, {{"__cxa_pure_virtual", __cxa_pure_virtual}, {"__cxa_guard_acquire", __cxa_guard_acquire}, {"__cxa_guard_release", __cxa_guard_release}});
        }
    }
    android_dlextinfo extinfo;
#ifdef __arm__
    // Workaround for v8 allocator crash Minecraft 1.16.100+ on a RaspberryPi2 running raspbian
    // Shadow some new overrides with host allocator fixes the crash
    // Seems to be unnecessary on a RaspberryPi4 running ubuntu arm64
    hooks.emplace_back(mcpelauncher_hook_t{"_Znaj", (void*)((void* (*)(std::size_t)) & ::operator new[])});
    hooks.emplace_back(mcpelauncher_hook_t{"_Znwj", (void*)((void* (*)(std::size_t)) & ::operator new)});
    hooks.emplace_back(mcpelauncher_hook_t{"_ZnwjSt11align_val_t", (void*)((void* (*)(std::size_t, std::align_val_t)) & ::operator new[])});
    // The Openssl cpuid setup seems to not work correctly and allways crashs with "invalid instruction" Minecraft 1.16.10 (beta 1.16.0.66) or lower
    // Shadowing it, avoids allways defining OPENSSL_armcap=0
    hooks.emplace_back(mcpelauncher_hook_t{"OPENSSL_cpuid_setup", (void*)+[]() -> void {}});
#endif

    for(auto&& e : preinitHooks) {
        hooks.emplace_back(mcpelauncher_hook_t{e.first.data(), e.second.value});
    }

    // Minecraft 1.16.210+ removes the symbols previously used to patch it via vtables, so use hooks instead if supplied
    if(showMousePointerCallback && hideMousePointerCallback) {
        hooks.emplace_back(mcpelauncher_hook_t{"_ZN11AppPlatform16showMousePointerEv", showMousePointerCallback});
        hooks.emplace_back(mcpelauncher_hook_t{"_ZN11AppPlatform16hideMousePointerEv", hideMousePointerCallback});
    }
    if(fullscreenCallback) {
        hooks.emplace_back(mcpelauncher_hook_t{"_ZN11AppPlatform17setFullscreenModeE14FullscreenMode", fullscreenCallback});
    }

    if(closeCallback) {
        hooks.emplace_back(mcpelauncher_hook_t{"GameActivity_finish", closeCallback});
    }

    static void* fmod = nullptr;
    // Temporary feature flag to disable native fmod patching
    if(!fmod && ReadEnvFlag("MCPELAUNCHER_PATCH_FMOD", true)) {
        fmod = linker::dlopen("libfmod.so", 0);
    }
    if(fmod) {
        if(linker::get_library_base(fmod)) {
            if(FmodUtils::setup(fmod)) {
                hooks.emplace_back(mcpelauncher_hook_t{"_ZN4FMOD6System4initEijPv", reinterpret_cast<void*>(&FmodUtils::initHook)});
                hooks.emplace_back(mcpelauncher_hook_t{"_ZN4FMOD6System9setOutputE15FMOD_OUTPUTTYPE", (void*)+[]() {
                                                           // stub to make the game use aaudio
                                                       }});
            }
        } else {
            linker::dlclose(fmod);
            fmod = nullptr;
        }
    }
    auto libc = linker::dlopen("libc.so", 0);
    // Detect Android Integrity Protection
    void* pairipcore = linker::dlopen("libpairipcore.so", 0);
    if(!pairipcore) {
        Log::error("MinecraftUtils", "Failed to load libpairipcore: %s", linker::dlerror());
    } else {
        Log::info("MinecraftUtils", "Loaded libpairipcore");
    }

    // webrtc shortcut
    auto bgetifaddrs = linker::dlsym(libc, "getifaddrs");
    if(bgetifaddrs) {
        hooks.emplace_back(mcpelauncher_hook_t{"_ZN3rtc10getifaddrsEPP7ifaddrs", bgetifaddrs});
    }
    auto bfreeifaddrs = linker::dlsym(libc, "freeifaddrs");
    if(bfreeifaddrs) {
        hooks.emplace_back(mcpelauncher_hook_t{"_ZN3rtc11freeifaddrsEP7ifaddrs", bfreeifaddrs});
    }
    if(ReadEnvFlag("MCPELAUNCHER_DISABLE_TELEMETRY", false)) {
        hooks.emplace_back(mcpelauncher_hook_t{"_ZN9Microsoft12Applications6Events19TelemetrySystemBase5startEv", (void*)+[]() {
            Log::error("MinecraftUtils", "TelemetrySystemBase::start");
        }});
    } else if(pairipcore) {
        // Phase 3 Step 1: Rust loads libsqliteX.so ELF, registers only sqlite3_* symbols
        auto sqlite3_path = PathHelper::findDataFile("lib/" + std::string(PathHelper::getAbiDir()) + "/libsqliteX.so");
        char* resolved = realpath(sqlite3_path.c_str(), nullptr);
        if(resolved) {
            std::string dir(resolved);
            auto pos = dir.rfind('/');
            if(pos != std::string::npos) {
                dir.resize(pos);
                linker_rust_add_search_path(dir.c_str());
            }
            free(resolved);
        }
        auto sqlite3 = linker_rust_dlopen_sqlite("libsqliteX.so");
        if(sqlite3) {
            Log::info("MinecraftUtils", "Rust linker loaded libsqliteX.so (handle=%zu)", sqlite3);
        } else {
            Log::error("MinecraftUtils", "Rust linker failed to load libsqliteX.so");
        }
    }

    hooks.emplace_back(mcpelauncher_hook_t{nullptr, nullptr});
    extinfo.flags = ANDROID_DLEXT_MCPELAUNCHER_HOOKS;
    extinfo.mcpelauncher_hooks = hooks.data();

    // Phase 2: Try Rust linker first (full load with relocations)
    void* handle = nullptr;
    size_t rust_handle = 0;
    {
        // Build parallel arrays for Rust FFI
        size_t hook_count = 0;
        for (auto& h : hooks) {
            if (h.name == nullptr) break;
            hook_count++;
        }
        std::vector<const char*> hook_names(hook_count);
        std::vector<void*> hook_vals(hook_count);
        for (size_t i = 0; i < hook_count; i++) {
            hook_names[i] = hooks[i].name;
            hook_vals[i] = hooks[i].value;
        }

        rust_handle = linker_rust_dlopen_ext("libminecraftpe.so", 0,
                                              hook_names.data(), hook_vals.data(),
                                              hook_count);
        if (rust_handle != 0) {
            Log::info("MinecraftUtils", "Rust linker loaded libminecraftpe.so (handle=%zu)", rust_handle);
        } else {
            Log::warn("MinecraftUtils", "Rust linker failed to load libminecraftpe.so, falling back to C++");
        }
    }

    // Phase 0 gate: MCPELAUNCHER_LINKER_RUST_ONLY=1 aborts if Rust linker failed
    if (rust_handle == 0) {
        const char* rust_only = getenv("MCPELAUNCHER_LINKER_RUST_ONLY");
        if (rust_only && rust_only[0] == '1' && rust_only[1] == '\0') {
            Log::error("MinecraftUtils", "MCPELAUNCHER_LINKER_RUST_ONLY=1 is set but Rust linker failed — aborting");
            abort();
        }
    }

    if (rust_handle != 0) {
        // Rust linker loaded the game — handle is a Rust handle (small int).
        // Hooks were applied during Rust relocation (external_symbols preferred).
        // All C++ APIs (dlsym, get_library_base, etc.) use mcpelauncher_dispatch_*
        // which resolve Rust handles to Rust FFI calls automatically.
        handle = reinterpret_cast<void*>(rust_handle);
        for(auto&& h : hooks) {
            if(h.name) {
                void* addr = mcpelauncher_dispatch_dlsym(handle, h.name);
                Log::trace("MinecraftUtils", "Found hook: %s @ %p (stub=%p)", h.name, addr, h.value);
                if(auto&& res = preinitHooks.find(h.name); res != preinitHooks.end() && res->second.callback != nullptr) {
                    Log::trace("MinecraftUtils", "with value: %p", h.value);
                    res->second.callback(res->second.user, h.value);
                }
            }
        }
        HookManager::instance.addLibrary(handle);
    } else {
        // Fallback to C++ loader
        handle = linker::dlopen_ext("libminecraftpe.so", 0, &extinfo);
        if(libc) {
            linker::dlclose(libc);
        }
        if(libcxx) {
            linker::dlclose(libcxx);
        }
        if(libstdcxx) {
            linker::dlclose(libstdcxx);
        }
        if(pairipcore) {
            linker::dlclose(pairipcore);
        }
        if(handle == nullptr) {
            Log::error("MinecraftUtils", "Failed to load Minecraft: %s", linker::dlerror());
        } else {
            if(fmod) {
                linker::dlclose(fmod);
            }
            for(auto&& h : hooks) {
                if(h.name) {
                    Log::trace("MinecraftUtils", "Found hook: %s @ %p", h.name, mcpelauncher_dispatch_dlsym(handle, h.name));
                    if(auto&& res = preinitHooks.find(h.name); res != preinitHooks.end() && res->second.callback != nullptr) {
                        Log::trace("MinecraftUtils", "with value: %p", h.value);
                        res->second.callback(res->second.user, h.value);
                    }
                }
            }
            HookManager::instance.addLibrary(handle);
        }
    }
    return handle;
}
const char* MinecraftUtils::getLibraryAbi() {
    return PathHelper::getAbiDir();
}

size_t MinecraftUtils::getLibraryBase(void* handle) {
    return mcpelauncher_dispatch_get_library_base(handle);
}



void MinecraftUtils::setupGLES2Symbols(void* (*resolver)(const char*)) {
    int i = 0;
    std::unordered_map<std::string, void*> syms;
    while(true) {
        const char* sym = glesv2_symbols[i];
        if(sym == nullptr)
            break;
        syms[sym] = resolver(sym);
        i++;
    }
    linker::load_library("libGLESv2.so", syms);
}

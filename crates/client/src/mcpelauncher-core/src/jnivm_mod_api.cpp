// Phase 6 surviving C++ shim: exposes `jnivm_register_method` to the Rust
// `MinecraftUtils::getApi` port. The `ModHandle`/`ModHandleNoVoid` MDO
// (see below) register native method/data-field implementations on the C++
// FakeJni `jnivm` VM. These touch C++-only types (`jnivm::Class`,
// `jnivm::Method`, `jnivm::impl::MethodHandleBase<T>`) that Rust cannot emit
// cleanly, so the machinery stays in C++ and is referenced by address from the
// Rust `getApi` map. Full `jnivm` integration is deferred to the JNI port
// (libjnivm-sys); until then this preserves the current boot/behavior.
//
// Extracted verbatim from the former getApi jnivm section (minecraft_utils.cpp:
// 299-593). NOT on the boot path (only mods invoke jnivm_register_method).

#include <jnivm.h>
#include <jni.h>
#include <algorithm>
#include <cstdarg>
#include <cstring>
#include <memory>
#include <string>
#include <vector>
#include <cstdio>
#include <dlfcn.h>
#include <errno.h>
#include <signal.h>
#include <unistd.h>
#include <sys/wait.h>

#include <log.h>

// Rust linker / path FFI (resolved at client final link).
extern "C" int mcpelauncher_dispatch_dladdr(const void* addr, void* info);
extern "C" const char* env_path_util_get_app_dir();
extern "C" const char* env_path_util_find_in_path(const char* what, const char* path, const char* cwd);

// ---- mcpelauncher_log / mcpelauncher_vlog (Log::log/vlog wrappers) ----
extern "C" void mc_mod_log(int level, const char* tag, const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    Log::vlog((LogLevel)level, tag, fmt, ap);
    va_end(ap);
}
extern "C" void mc_mod_vlog(int level, const char* tag, const char* fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    Log::vlog((LogLevel)level, tag, fmt, ap);
    va_end(ap);
}

// ---- Google credentials helper (uses __builtin_return_address, uid trick) ----
struct GoogleCredentials {
    const char* email;
    const char* token;
};

static std::vector<const char*> convertToC(std::vector<std::string> const& v) {
    std::vector<const char*> ret;
    for (auto const& i : v)
        ret.push_back(i.c_str());
    ret.push_back(nullptr);
    return std::move(ret);
}

static std::string getUiExecutablePath() {
#ifndef MCPELAUNCHER_UI_PATH
#define MCPELAUNCHER_UI_PATH "."
#endif
    const char* path = env_path_util_find_in_path("mcpelauncher-ui-qt", MCPELAUNCHER_UI_PATH, env_path_util_get_app_dir());
    if (path != nullptr)
        return std::string(path);
    path = env_path_util_find_in_path("mcpelauncher-ui-qt", nullptr, nullptr);
    if (path != nullptr)
        return std::string(path);
    return "mcpelauncher-ui-qt";
}

extern "C" void mc_mod_request_google_credentials(void (*onsuccess)(GoogleCredentials), void (*onfailure)(const char* error)) {
    const void* caller_addr = __builtin_return_address(0);
    Dl_info info;
    if(mcpelauncher_dispatch_dladdr(caller_addr, &info) != 0) {
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

// ---- jnivm ModHandle machinery ----

// ---- ModHandle: a jnivm MethodHandle backed by a jvalue-marshalling fn ----
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

/// Extern "C" entry point registered by the Rust getApi under
/// `jnivm_register_method`. Signature mirrors the C++ getApi lambda.
extern "C" bool mc_mod_jnivm_register_method(JNIEnv* env, jclass cl, int type, const char* name, const char* signature, jvalue (*cbk)(JNIEnv* env, jobject thiz, jvalue* values)) {
    if(!env || !cl || !name || !signature) {
        return false;
    }
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
}
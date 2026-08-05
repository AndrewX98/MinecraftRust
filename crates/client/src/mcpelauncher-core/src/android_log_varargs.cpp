// Phase 7: varargs-only remainder of hybris_android_log_hook.cpp.
// Stable Rust cannot define `...`/va_list extern "C" functions, so the three
// varargs __android_log_* entry points live here; the level mapping is
// single-sourced in Rust (corelib mc_android_convert_log_level) and the
// non-varargs __android_log_write is Rust-owned (client android_log_hook.rs).
// Non-static: capi.cpp takes these addresses to register the liblog.so stub
// symbols with the Rust linker.
#include <log.h>
#include <cstdarg>
#include <cstdlib>

extern "C" int mc_android_convert_log_level(int level);

extern "C" void __android_log_vprint(int prio, const char *tag, const char *fmt, va_list args) {
    Log::vlog((LogLevel) mc_android_convert_log_level(prio), tag, fmt, args);
}
extern "C" void __android_log_print(int prio, const char *tag, const char *fmt, ...) {
    va_list args;
    va_start(args, fmt);
    __android_log_vprint(prio, tag, fmt, args);
    va_end(args);
}
extern "C" void __android_log_assert(const char* cond, const char* tag, const char* fmt, ...) {
    va_list args;
    va_start(args, fmt);
    if (fmt) {
        Log::vlog(LogLevel::LOG_ERROR, tag, fmt, args);
    } else {
        Log::log(LogLevel::LOG_ERROR, tag, "Assertion failed: %s", cond);
    }
    va_end(args);
    abort();
}

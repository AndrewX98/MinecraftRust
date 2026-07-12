/// Stub replacing fake_egl.cpp for the Rust build.
/// Delegates all EGL function implementations and FakeEGL methods to Rust.
/// Also provides a linker::load_library wrapper so Rust can register symbols.

#include "fake_egl.h"
#include <mcpelauncher/linker.h>
#include <dlfcn.h>
#include <cstring>

// --- linker::load_library wrapper for Rust ---
// Takes parallel arrays of (name, func_ptr) and forwards to C++ linker,
// also mirroring to the Rust linker state.
extern "C" size_t linker_load_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);

extern "C" void linker_load_library(const char* name, const char* const* names, void* const* funcs, int count) {
    std::unordered_map<std::string, void*> syms;
    for (int i = 0; i < count; i++) {
        syms[names[i]] = funcs[i];
    }
    linker::load_library(name, syms);
    // Mirror to Rust linker state
    linker_load_library_rust(name, names, funcs, (size_t)count);
}

// --- Rust EGL function implementations ---
extern "C" {
    int fake_egl_initialize(void* display, int* major, int* minor);
    int fake_egl_terminate(void* display);
    int fake_egl_get_error();
    const char* fake_egl_query_string(void* display, int name);
    void* fake_egl_get_display(void* native);
    void* fake_egl_get_current_display();
    void* fake_egl_get_current_context();
    int fake_egl_choose_config(void* display, const int* attrib_list, void* configs, int config_size, int* num_config);
    int fake_egl_get_config_attrib(void* display, void* config, int attribute, int* value);
    void* fake_egl_create_window_surface(void* display, void* config, void* native_window, const int* attrib_list);
    int fake_egl_destroy_surface(void* display, void* surface);
    void* fake_egl_create_context(void* display, void* config, void* share_context, const int* attrib_list);
    int fake_egl_destroy_context(void* display, void* context);
    int fake_egl_make_current(void* display, void* draw, void* read, void* context);
    int fake_egl_swap_buffers(void* display, void* surface);
    int fake_egl_swap_interval(void* display, int interval);
    int fake_egl_query_surface(void* display, void* surface, int attribute, int* value);
    void* fake_egl_get_proc_address(const char* name);

    // FakeEGL class methods
    void fake_egl_set_proc_addr_function(void* (*fn)(const char*));
    void fake_egl_install_library();
    void fake_egl_setup_gl_overrides();
    void fake_egl_save_current_window_handle();
    void fake_egl_save_native_window(unsigned long window);
    void fake_egl_release_context();
    void fake_egl_add_swap_buffers_callback(void* user, void (*callback)(void*, void*, void*));
}

// --- FakeEGL class method delegations ---

bool FakeEGL::enableTexturePatch = false;
std::vector<FakeEGL::SwapBuffersCallback> FakeEGL::swapBuffersCallbacks = {};
std::mutex FakeEGL::swapBuffersCallbacksLock;

void FakeEGL::setProcAddrFunction(void* (*fn)(const char*)) {
    fake_egl_set_proc_addr_function(fn);
}

void FakeEGL::installLibrary() {
    fake_egl_install_library();
}

void FakeEGL::setupGLOverrides() {
    fake_egl_setup_gl_overrides();
}

void FakeEGL::saveCurrentWindowHandle() {
    fake_egl_save_current_window_handle();
}

void FakeEGL::saveNativeWindow(unsigned long window) {
    fake_egl_save_native_window(window);
}

void FakeEGL::releaseContext() {
    fake_egl_release_context();
}

void FakeEGL::addSwapBuffersCallback(void* user, void (*callback)(void*, void*, void*)) {
    fake_egl_add_swap_buffers_callback(user, callback);
}

// --- namespace fake_egl function delegations ---

namespace fake_egl {

EGLBoolean eglInitialize(EGLDisplay display, EGLint* major, EGLint* minor) {
    return fake_egl_initialize(display, major, minor);
}

EGLBoolean eglTerminate(EGLDisplay display) {
    return fake_egl_terminate(display);
}

EGLint eglGetError() {
    return fake_egl_get_error();
}

char const* eglQueryString(EGLDisplay display, EGLint name) {
    return fake_egl_query_string(display, name);
}

EGLDisplay eglGetDisplay(EGLNativeDisplayType dp) {
    return (EGLDisplay)fake_egl_get_display((void*)dp);
}

EGLDisplay eglGetCurrentDisplay() {
    return (EGLDisplay)fake_egl_get_current_display();
}

EGLContext eglGetCurrentContext() {
    return (EGLContext)fake_egl_get_current_context();
}

EGLBoolean eglChooseConfig(EGLDisplay display, EGLint const* attrib_list, EGLConfig* configs, EGLint config_size, EGLint* num_config) {
    return fake_egl_choose_config(display, attrib_list, configs, config_size, num_config);
}

EGLBoolean eglGetConfigAttrib(EGLDisplay display, EGLConfig config, EGLint attribute, EGLint* value) {
    return fake_egl_get_config_attrib(display, config, attribute, value);
}

EGLSurface eglCreateWindowSurface(EGLDisplay display, EGLConfig config, EGLNativeWindowType native_window, EGLint const* attrib_list) {
    return (EGLSurface)fake_egl_create_window_surface(display, config, (void*)native_window, attrib_list);
}

EGLBoolean eglDestroySurface(EGLDisplay display, EGLSurface surface) {
    return fake_egl_destroy_surface(display, surface);
}

EGLContext eglCreateContext(EGLDisplay display, EGLConfig config, EGLContext share_context, EGLint const* attrib_list) {
    return (EGLContext)fake_egl_create_context(display, config, share_context, attrib_list);
}

EGLBoolean eglDestroyContext(EGLDisplay display, EGLContext context) {
    return fake_egl_destroy_context(display, context);
}

EGLBoolean eglMakeCurrent(EGLDisplay display, EGLSurface draw, EGLSurface read, EGLContext context) {
    return fake_egl_make_current(display, draw, read, context);
}

EGLBoolean eglSwapBuffers(EGLDisplay display, EGLSurface surface) {
    return fake_egl_swap_buffers(display, surface);
}

EGLBoolean eglSwapInterval(EGLDisplay display, EGLint interval) {
    return fake_egl_swap_interval(display, interval);
}

EGLBoolean eglQuerySurface(EGLDisplay display, EGLSurface surface, EGLint attribute, EGLint* value) {
    return fake_egl_query_surface(display, surface, attribute, value);
}

void* eglGetProcAddress(const char* name) {
    return fake_egl_get_proc_address(name);
}

} // namespace fake_egl

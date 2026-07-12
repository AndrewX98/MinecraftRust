/// Minimal stub replacing core_patches.cpp for the Rust build.
/// Holds C++ type-dependent state (shared_ptr<WindowCallbacks>,
/// std::function) and provides extern "C" entry points for Rust/FFI.

#include "core_patches.h"
#include "window_callbacks.h"
#include "fake_egl.h"
#include <game_window.h>
#include <mcpelauncher/linker.h>
#include <mcpelauncher/patch_utils.h>
#include <log.h>
#include <memory>
#include <vector>
#include <functional>
#include <unordered_map>
#include <string>

// Static variable definitions (private members, but defined here)
CorePatches::GameWindowHandle CorePatches::currentGameWindowHandle;
std::vector<std::function<void()>> CorePatches::onWindowCreatedCallbacks;

// --- Rust functions (declared) ---
extern "C" void core_patches_install_impl(void* handle);
extern "C" void linker_add_symbols_to_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);

// --- extern "C" helpers for Rust FFI ---

extern "C" void* core_linker_dlsym(void* handle, const char* sym) {
    return linker::dlsym(handle, sym);
}

extern "C" void core_vtable_replace(void* lib, void** vta, const char* name, void* replacement) {
    PatchUtils::VtableReplaceHelper vtr(lib, vta, vta);
    vtr.replace(name, replacement);
}

// --- CorePatches member functions (have private access) ---

void CorePatches::showMousePointer() {
    currentGameWindowHandle.mouseLocked = false;
    if (currentGameWindowHandle.callbacks)
        currentGameWindowHandle.callbacks->setCursorLocked(false);
}

void CorePatches::hideMousePointer() {
    currentGameWindowHandle.mouseLocked = true;
    if (currentGameWindowHandle.callbacks)
        currentGameWindowHandle.callbacks->setCursorLocked(true);
}

bool CorePatches::isMouseLocked() {
    return currentGameWindowHandle.mouseLocked;
}

void CorePatches::setFullscreen(void* t, bool fullscreen) {
    if (currentGameWindowHandle.callbacks)
        currentGameWindowHandle.callbacks->setFullscreen(fullscreen);
}

void CorePatches::setPendingDelayedPaste() {
    if (currentGameWindowHandle.callbacks)
        currentGameWindowHandle.callbacks->setDelayedPaste();
}

void CorePatches::install(void* handle) {
    core_patches_install_impl(handle);
}

void CorePatches::setGameWindow(std::shared_ptr<GameWindow> gameWindow) {
    currentGameWindowHandle.window = std::move(gameWindow);
}

void CorePatches::setGameWindowCallbacks(std::shared_ptr<WindowCallbacks> gameWindowCallbacks) {
    currentGameWindowHandle.callbacks = std::move(gameWindowCallbacks);
    for (size_t i = 0; i < onWindowCreatedCallbacks.size(); i++) {
        onWindowCreatedCallbacks[i]();
    }
}

void CorePatches::loadGameWindowLibrary() {
    std::unordered_map<std::string, void*> syms;

    syms["game_window_get_primary_window"] = (void*)+[]() -> CorePatches::GameWindowHandle* {
        return &CorePatches::currentGameWindowHandle;
    };

    syms["game_window_is_mouse_locked"] = (void*)+[](CorePatches::GameWindowHandle* handle) -> bool {
        return handle->mouseLocked;
    };

    syms["game_window_get_input_mode"] = (void*)+[](CorePatches::GameWindowHandle* handle) -> int {
        return (int)handle->callbacks->getInputMode();
    };

    syms["game_window_add_keyboard_callback"] = (void*)+[](CorePatches::GameWindowHandle* handle, void* user, bool (*callback)(void* user, int keyCode, int action)) {
        handle->callbacks->addKeyboardCallback(user, callback);
    };

    syms["game_window_add_mouse_button_callback"] = (void*)+[](CorePatches::GameWindowHandle* handle, void* user, bool (*callback)(void* user, double x, double y, int button, int action)) {
        handle->callbacks->addMouseButtonCallback(user, callback);
    };

    syms["game_window_add_mouse_position_callback"] = (void*)+[](CorePatches::GameWindowHandle* handle, void* user, bool (*callback)(void* user, double x, double y, bool relative)) {
        handle->callbacks->addMousePositionCallback(user, callback);
    };

    syms["game_window_add_mouse_scroll_callback"] = (void*)+[](CorePatches::GameWindowHandle* handle, void* user, bool (*callback)(void* user, double x, double y, double dx, double dy)) {
        handle->callbacks->addMouseScrollCallback(user, callback);
    };

    syms["game_window_add_window_creation_callback"] = (void*)+[](void* user, void (*onCreated)(void* user)) {
        CorePatches::onWindowCreatedCallbacks.emplace_back(std::bind(onCreated, user));
    };

    syms["game_window_add_swap_buffers_callback"] = (void*)+[](void* user, void (*callback)(void* user, EGLDisplay display, EGLSurface surface)) {
        FakeEGL::addSwapBuffersCallback(user, callback);
    };

    linker::load_library("libmcpelauncher_gamewindow.so", syms);
    // Mirror symbols to Rust linker state (lib already registered from capi.cpp)
    {
        size_t n = syms.size();
        if (n > 0) {
            std::vector<const char*> keys(n);
            std::vector<void*> vals(n);
            size_t i = 0;
            for (auto& [k, v] : syms) {
                keys[i] = k.c_str();
                vals[i] = v;
                i++;
            }
            linker_add_symbols_to_library_rust("libmcpelauncher_gamewindow.so", keys.data(), vals.data(), n);
        }
    }
}

// --- extern "C" thunks for Rust / FFI ---
// These simply forward to the CorePatches member functions.

extern "C" void core_patches_show_mouse_pointer() {
    CorePatches::showMousePointer();
}
extern "C" void core_patches_hide_mouse_pointer() {
    CorePatches::hideMousePointer();
}
extern "C" void core_patches_set_fullscreen(void* t, bool fs) {
    CorePatches::setFullscreen(t, fs);
}
extern "C" bool core_patches_is_mouse_locked() {
    return CorePatches::isMouseLocked();
}
extern "C" void core_patches_set_pending_delayed_paste() {
    CorePatches::setPendingDelayedPaste();
}
extern "C" void core_patches_install(void* handle) {
    core_patches_install_impl(handle);
}

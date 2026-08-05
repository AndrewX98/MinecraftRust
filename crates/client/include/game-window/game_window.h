#pragma once

// Phase 5: the C++ `mcpelauncher-gamewindow` lib was deleted (window owned by
// Rust eglut, see crate::game_window.rs). Only the enums consumed by remaining
// C++ sources survive here. `GameWindow`/`GameWindowManager` classes are gone;
// the game-visible window is an opaque token (the eglut X11 window id).

#include <string>
#include "key_mapping.h"

enum class GraphicsApi {
    OPENGL,
    OPENGL_ES2
};
enum class KeyAction {
    PRESS,
    REPEAT,
    RELEASE
};
enum class MouseButtonAction {
    PRESS,
    RELEASE
};

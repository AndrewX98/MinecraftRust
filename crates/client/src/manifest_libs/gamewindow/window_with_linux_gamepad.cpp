#include "window_with_linux_gamepad.h"

// Gamepad glue removed in the pure-Rust port: window registration, per-frame
// polling and focus tracking now live in crates/client/src/gamepad/ and are
// driven from the Rust eglut event loop.

WindowWithLinuxJoystick::WindowWithLinuxJoystick(std::string const& title, int width, int height, GraphicsApi api) :
        GameWindow(title, width, height, api) {
}

WindowWithLinuxJoystick::~WindowWithLinuxJoystick() {
}

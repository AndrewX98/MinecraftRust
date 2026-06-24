/// Minimal stub replacing settings.cpp for the Rust build.
/// Settings::load/save are not needed because main.cpp (which calls load()) is
/// excluded from the Rust build. Only the static variable definitions that
/// compiled C++ files reference are provided here.

#include "settings.h"

// Provide definitions for the static members referenced by compiled C++ files.
// Members only used by imgui_ui.cpp (not compiled) are omitted — no definition
// needed since no TU odr-uses them.

std::atomic<int> Settings::menubarsize = {0};
std::string Settings::clipboard;
bool Settings::enable_keyboard_autofocus_patches_1_20_60 = false;
bool Settings::enable_keyboard_autofocus_paste_patches_1_20_60 = false;
float Settings::scale = 1.0f;
bool Settings::fullscreen = false;

// No-op: load() is never called in the Rust build (main.cpp excluded),
// so saving would overwrite the user's real settings with defaults.
// If save functionality is needed, implement mc_settings_save() in Rust.
void Settings::save() {
    // no-op
}

std::string Settings::getPath() { return ""; }
void Settings::load() {}

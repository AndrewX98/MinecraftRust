#include <game_window.h>
#include <properties/property_list.h>
#include "symbols.h"
#include "splitscreen_patch.h"
#include "shader_error_patch.h"

struct LauncherOptions {
    int windowWidth, windowHeight;
    bool useStdinImport;
    bool emulateTouch;
    GraphicsApi graphicsApi;
    std::string importFilePath;
    std::string sendUri;
};

LauncherOptions options = {1200, 800, false, false, GraphicsApi::OPENGL_ES2, "", ""};

// Stubs for symbols from files excluded from build (symbols.cpp)
bool Keyboard::useLegacyKeyboard = false;
int* Keyboard::_states = nullptr;
std::vector<Keyboard::InputEvent>* Keyboard::_inputs = nullptr;
std::vector<Keyboard::LegacyInputEvent>* Keyboard::_inputsLegacy = nullptr;
int* Keyboard::_gameControllerId = nullptr;
void (*Mouse::feed)(char, char, short, short, short, short) = nullptr;

// Stubs for patch files excluded from build (splitscreen_patch.cpp, shader_error_patch.cpp)
void SplitscreenPatch::onGLContextCreated() {}
void ShaderErrorPatch::onGLContextCreated() {}

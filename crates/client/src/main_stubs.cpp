#include "main.h"
#include "splitscreen_patch.h"
#include "shader_error_patch.h"

LauncherOptions options = {1200, 800, false, false, GraphicsApi::OPENGL_ES2, "", ""};

// Stubs for patch files excluded from build (splitscreen_patch.cpp, shader_error_patch.cpp)
void SplitscreenPatch::onGLContextCreated() {}
void ShaderErrorPatch::onGLContextCreated() {}

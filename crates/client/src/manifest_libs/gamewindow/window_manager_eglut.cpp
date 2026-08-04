#define EGLUT_NO_X11_INCLUDE
#include "window_manager_eglut.h"
#include "window_eglut.h"
#include <eglut.h>
#include <eglut_x11.h>
#include <unistd.h>
#ifndef __FreeBSD__
#include <linux/limits.h>
#endif
#include <libgen.h>
#include <cstring>

extern "C" void eglGetProcAddress();
// Pure-Rust gamepad mapping loaders (crates/client/src/gamepad/).
extern "C" void gamepad_load_mappings_from_file(const char* path);
extern "C" void gamepad_load_mappings(const char* content);

EGLUTWindowManager::EGLUTWindowManager() {
    char buf[PATH_MAX];
    memset(buf, 0, sizeof(buf));
    readlink("/proc/self/exe", buf, sizeof(buf) - 1);
    eglutInitX11ClassInstanceName(basename(buf));
    eglutInit(0, nullptr); // the args aren't really required and are troublesome to pass with this system
}

GameWindowManager::ProcAddrFunc EGLUTWindowManager::getProcAddrFunc() {
    return (GameWindowManager::ProcAddrFunc) eglGetProcAddress;
}

std::shared_ptr<GameWindow> EGLUTWindowManager::createWindow(const std::string& title, int width, int height,
                                                             GraphicsApi api) {
    return std::shared_ptr<GameWindow>(new EGLUTWindow(title, width, height, api));
}

void EGLUTWindowManager::addGamepadMappingFile(const std::string &path) {
    gamepad_load_mappings_from_file(path.c_str());
}

void EGLUTWindowManager::addGamePadMapping(const std::string &content) {
    gamepad_load_mappings(content.c_str());
}

#ifndef FALLBACK_EGLUT
// Define this window manager as the used one
std::shared_ptr<GameWindowManager> GameWindowManager::createManager() {
    return std::shared_ptr<GameWindowManager>(new EGLUTWindowManager());
}
#endif
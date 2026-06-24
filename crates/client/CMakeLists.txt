cmake_minimum_required(VERSION 3.10)
project(mcpelauncher-rust-client LANGUAGES CXX C)

# Paths
set(MANIFEST_DIR "${CMAKE_CURRENT_SOURCE_DIR}/../../mcpelauncher-manifest")

# Include directories
set(INCLUDE_DIRS
    "${MANIFEST_DIR}/libjnivm/include"
    "${MANIFEST_DIR}/libjnivm/src"
    "${MANIFEST_DIR}/mcpelauncher-linker/include"
    "${MANIFEST_DIR}/mcpelauncher-linker/public_include"
    "${MANIFEST_DIR}/mcpelauncher-core/include"
    "${MANIFEST_DIR}/mcpelauncher-client/src"
    "${MANIFEST_DIR}/build/mcpelauncher-client/build_info"
    "${MANIFEST_DIR}/imgui"
    "${MANIFEST_DIR}/imgui/backends"
)

# Find system packages
find_package(CURL REQUIRED)
find_package(OpenSSL REQUIRED)
find_package(PkgConfig)
if(PKG_CONFIG_FOUND)
    pkg_check_modules(SDL2 sdl2)
    pkg_check_modules(PULSEAUDIO libpulse libpulse-simple)
endif()

# Client source files (everything except main.cpp)
set(CLIENT_SOURCES
    src/capi.cpp
    "${MANIFEST_DIR}/mcpelauncher-client/src/window_callbacks.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/xbox_live_helper.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/splitscreen_patch.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/strafe_sprint_patch.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/fake_swappygl.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/cll_upload_auth_step.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/gl_core_patch.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/hbui_patch.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/shader_error_patch.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/fake_looper.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/fake_window.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/fake_assetmanager.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/fake_egl.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/fake_inputqueue.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/symbols.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/text_input_handler.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/core_patches.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/thread_mover.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/util.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/xal_webview_factory.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/settings.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/xal_webview_qt.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/cpuid.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/xbox_shutdown_patch.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/texel_aa_patch.cpp"
    # JNI files
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/jni_descriptors.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/main_activity.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/asset_manager.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/playfab.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/store.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/cert_manager.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/http_stub.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/package_source.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/jni_support.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/fmod.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/xbox_live.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/lib_http_client.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/lib_http_client_websocket.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/accounts.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/arrays.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/jbase64.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/locale.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/securerandom.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/signature.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/uuid.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/webview.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/shahasher.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/jni/ecdsa.cpp"
    # ImGui
    "${MANIFEST_DIR}/imgui/imgui.cpp"
    "${MANIFEST_DIR}/imgui/imgui_demo.cpp"
    "${MANIFEST_DIR}/imgui/imgui_draw.cpp"
    "${MANIFEST_DIR}/imgui/imgui_tables.cpp"
    "${MANIFEST_DIR}/imgui/imgui_widgets.cpp"
    "${MANIFEST_DIR}/imgui/backends/imgui_impl_opengl3.cpp"
    "${MANIFEST_DIR}/mcpelauncher-client/src/glad.c"
    "${MANIFEST_DIR}/mcpelauncher-client/src/imgui_ui.cpp"
)

# Create the library
add_library(mcpelauncher-rust-client STATIC ${CLIENT_SOURCES})
target_include_directories(mcpelauncher-rust-client PRIVATE ${INCLUDE_DIRS} ${CURL_INCLUDE_DIRS} ${OPENSSL_INCLUDE_DIR})
target_compile_definitions(mcpelauncher-rust-client PRIVATE
    USE_IMGUI
    IMGUI_IMPL_OPENGL_ES3
    MCPELAUNCHER_ENABLE_ERROR_WINDOW
)
target_compile_options(mcpelauncher-rust-client PRIVATE -fvisibility=hidden -w)
set_target_properties(mcpelauncher-rust-client PROPERTIES POSITION_INDEPENDENT_CODE ON)

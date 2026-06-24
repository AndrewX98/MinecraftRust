# C++ Bridge Interface

## Rust → C++ (extern "C" declarations)

Rust declares these extern "C" functions and calls them through FFI. They are implemented in C++ bridge files.

### In `capi.rs` (15 functions)

| Rust Call | C++ / Rust Target | File | Purpose |
|-----------|-------------------|------|---------|
| `mc_setup_paths` | `pathhelper_setGameDir/DataDir/CacheDir` | capi.cpp | Set game/data/cache dirs |
| `mc_init_version` | `MinecraftVersion::init` | capi.cpp | Init version info |
| `mc_get_libc_symbols` | `MinecraftUtils::getLibCSymbols` | capi.cpp | Get merged libc symbols |
| `mc_load_core_libraries` | `linker::init` + `loadLibM` + `setupHybris` | capi.cpp | Init linker, load core libs |
| `mc_load_minecraft` | `MinecraftUtils::loadMinecraftLib` | capi.cpp | Load libminecraftpe.so |
| `mc_setup_android_hooks` | FakeLooper/FakeAssetManager/... + FakeEGL::installLibrary | jni_bridge_stub.cpp | Register android hooks |
| `mc_create_window_and_setup_graphics` | EGLUT window + GLES2 symbol setup | jni_bridge_stub.cpp | Create window, resolve GL |
| `mc_egl_swap_buffers` | `fake_egl::eglSwapBuffers` | jni_bridge_stub.cpp | EGL swap (→ Rust) |
| `mc_dlsym` | `linker::dlsym` | capi.cpp | Resolve game symbol |
| `jni_support_create_cpp` | `jni_support_create_cpp()` (Rust → `jni_support_new_cpp()`) | jni_support.rs | Create C++ JniSupport |
| `jni_support_destroy_cpp` | `jni_support_destroy_cpp()` (Rust → `jni_support_delete()`) | jni_support.rs | Destroy C++ JniSupport |
| `jni_support_register_minecraft_natives_cpp` | `JniSupport::registerMinecraftNatives()` | jni_bridge_stub.cpp | Register game native methods |
| `fake_looper_set_jni_support` | Set C++ JniSupport on FakeLooper | jni_bridge_stub.cpp | Connect FakeLooper to C++ JVM |
| `fake_looper_set_rust_jni_support` | Set Rust JniSupport on FakeLooper | jni_bridge_stub.cpp | Connect FakeLooper to Rust JVM |
| `fake_assetmanager_create_and_set_global` | `FakeAssetManager::setGlobalAssetManager` | jni_bridge_stub.cpp | Create global asset mgr |

### In `jni_support.rs` (7 functions + dispatch)

| Rust Call | C++ / Rust Target | File | Purpose |
|-----------|-------------------|------|---------|
| `register_all_jnivm_classes(env)` | `jnivm_class_wrappers.cpp` | C++ wrapper | Register 10 Java classes with libjnivm-sys |
| `jnivm_set_main_window(window)` | `jnivm_class_wrappers.cpp` | C++ wrapper | Set global window ptr for C++ wrappers |
| `jnivm_set_storage_dir(dir)` | `jnivm_class_wrappers.cpp` | C++ wrapper | Set storage dir for MainActivity wrappers |
| `jnivm_set_asset_manager(mgr)` | `jnivm_class_wrappers.cpp` | C++ wrapper | Set asset manager for wrappers |
| `jnivm_set_stbi_load_from_memory(fn)` | `jnivm_class_wrappers.cpp` | C++ wrapper | Set stbi loader ptr |
| `jnivm_set_stbi_image_free(fn)` | `jnivm_class_wrappers.cpp` | C++ wrapper | Set stbi free ptr |
| `fake_jni_jvm_attach_library(jvm, path)` | Baron JVM `attachLibrary()` | jni_bridge_stub.cpp | Attach lib for JNI_OnLoad |
| `fake_jni_local_frame_create/destroy/get_env` | Baron `LocalFrame` | jni_bridge_stub.cpp | Baron local frame mgmt |
| `fake_assetmanager_get_instance()` | FakeAssetManager | C++ | Get global asset manager instance |

### In `rust_bridge.rs` (pure Rust)

All functions in `rust_bridge.rs` are Rust implementations that either:
- Stub out C++ functionality (FakeWindow, FakeSwappyGL, ThreadMover)
- Provide `#[no_mangle]` entry points called from C++ (via `core_patches_stub.cpp`, `jni_bridge_stub.cpp`)

Key `#[no_mangle]` functions callable from C++:
- `fake_window_set_size`, `fake_window_set_menubar_size` (stub window state)
- `fake_anativewindow_getwidth/height` (stub ANativeWindow)
- `fake_swappygl_fill_hooks` (stub SwappyGL hooks)
- `core_patches_show_mouse_pointer`, `core_patches_hide_mouse_pointer`, `core_patches_set_fullscreen` (callback targets)
- `fake_egl_*` (~30 functions: initialize, terminate, get_error, query_string, get_display, choose_config, create_window_surface, create_context, make_current, swap_buffers, etc.)
- `mc_glcorepatch_*` (~7 functions: install, install_gl, shader_source, link_program, use_program, bind_buffer)
- `shahasher_*`, `securerandom_generate_bytes_rust`, `jbase64_decode_rust`, `base64_encode_rust`, `file_util_read_file_rust`, `arrays_copy_of_range_rust` (~9 utility functions)

## C++ → Rust (`#[no_mangle]` extern "C" definitions)

Rust provides ~154+ `#[no_mangle]` extern "C" functions callable from C++.

### By Module

| Module | Count | Functions |
|--------|-------|-----------|
| `rust_bridge.rs` | ~62 | FakeWindow(4), SwappyGL(16), ThreadMover(2), GLCorePatch(7), CorePatches(1), WindowCallbacks(3), FakeEGL(~30), SHA/Base64/File(9), JNI variants |
| `jni_support.rs` | ~14 | jni_support_new/destroy/register_natives/start_game_with_baron/start_game/set_looper_running/on_window_created/on_window_closed/on_window_resized/send_key_down/send_key_up/send_motion_event/create_cpp/destroy_cpp |
| `fake_looper.rs` | ~7 | mc_register_fake_looper_hooks, fake_looper_prepare_begin, fake_looper_notify_window_created, fake_looper_create_window_callbacks, fake_looper_register_core_patches, fake_looper_show_window, fake_looper_*patch* |
| `eglut/` | ~60 | eglutInit/CreateWindow/PollEvents/MainLoop/WarpMousePointer, window mgmt, callbacks, mouse, compat, egl, event, state, xinput |
| `file_picker.rs` | ~8 | File picker factory CRUD |
| `libc-shim` | ~3 | get_shimmed_symbols_fill/len, shim_internal_rewrite_path |
| `libjnivm-sys` | ~9 | jnivm_create_vm/destroy_vm/get_env/find_class/get_method_id/.../register_natives |
| `linker` | ~3 | linker_init_rust/load_library_rust/show_state_rust |

### Key Categories

**FakeEGL** (~30 functions in `rust_bridge.rs:595-1424`):
- `fake_egl_initialize`, `_terminate`, `_get_error`, `_query_string`
- `fake_egl_get_display`, `_get_current_display`, `_get_current_context`
- `fake_egl_choose_config`, `_get_config_attrib`
- `fake_egl_create_window_surface`, `_destroy_surface`
- `fake_egl_create_context`, `_destroy_context`
- **`fake_egl_make_current`** — the critical black screen fix
- **`fake_egl_swap_buffers`** — dispatches to real EGL via TLS
- `fake_egl_get_proc_address`, `_swap_interval`, `_query_surface`
- `fake_egl_install_library`, `_setup_gl_overrides`, `_release_context`

**GLCorePatch** (~7 functions):
- `mc_glcorepatch_install`, `_install_gl`
- `mc_glcorepatch_gl_shader_source` — replaces `#version 300 es` with `#version 410`
- `mc_glcorepatch_gl_link_program` — auto-generates VAO
- `mc_glcorepatch_gl_use_program`, `_gl_bind_buffer`

**Utility** (~9 functions):
- `shahasher_init_rust`, `shahasher_add_bytes_rust`, `shahasher_sign_hash_rust`, `shahasher_free_rust`
- `securerandom_generate_bytes_rust`
- `jbase64_decode_rust`, `base64_encode_rust`
- `file_util_read_file_rust`
- `arrays_copy_of_range_rust`

## Bridge Files (compiled by build.rs)

All located in `MinecraftRust/crates/client/src/`. Files where the C++ logic has been ported to Rust remain compiled as stubs to satisfy linker dependencies:

| File | Lines | Role |
|------|-------|------|
| `capi.cpp` | 213 | Low-level bridge: path setup, linker init, GLES2 symbol registration |
| `jni_bridge_stub.cpp` | 375 | Android hooks, window creation, game lib loading, C++ JniSupport FFI wrappers, FakeJni/Baron LocalFrame wrappers |
| `jnivm_class_wrappers.cpp` | 647 | Registers 10 Java classes with libjnivm-sys (FindClass + RegisterNatives) |
| `window_callbacks_stub.cpp` | 710 | Window callback registration, key mapping, delegates to Rust event dispatch |
| `core_patches_stub.cpp` | 141 | CorePatches vtable patching, cursor lock, fullscreen |
| `fake_egl_stub.cpp` | 161 | Delegates all EGL functions to Rust eglut module |
| `fake_looper_stub.cpp` | 152 | C++ helpers for Rust FakeLooper (prepare_begin, notify_window_created, create_window_callbacks, register_core_patches, show_window, poll helpers) |
| `fake_inputqueue_stub.cpp` | 112 | Full FakeInputQueue implementation |
| `fake_assetmanager_stub.cpp` | 214 | Full FakeAssetManager implementation |
| `text_input_handler_stub.cpp` | 233 | C++ TextInputHandler class |
| `main_stubs.cpp` | 28 | Stub data for Keyboard/Mouse/SplitscreenPatch globals |
| 15+ other stub files | ~500 | Minimal stubs for excluded JNI files (`_stub.cpp` for ecdsa, signature, cert_manager, http_stub, jbase64, arrays, asset_manager, package_source, securerandom, accounts, locale, playfab, fmod, webview, shahasher, file_picker, settings, cll_upload_auth_step, xal_webview_factory, xbox_live_helper) |

### Notable Ports to Rust

| Functionality | C++ Removed | Rust Replacement | Status |
|--------------|-------------|------------------|--------|
| Startup orchestration | `JniSupport::startGame()` | `jni_support::jni_support_start_game()` | Done |
| Event dispatch (sendKeyDown/Up/MotionEvent) | `JniSupport::sendKeyDown()` etc. | `jni_support::jni_support_send_key_down()` etc. | Done |
| JniSupport create/destroy | `new/delete JniSupport` | `jni_support_create_cpp()` / `jni_support_destroy_cpp()` | Done |
| FakeLooper prepare | `FakeLooper::prepare()` | `fake_looper::prepare()` (Rust) | Done |
| FakeLooper addFd | `FakeLooper::addFd()` | `fake_looper::add_fd()` (Rust) | Done |
| FakeLooper pollAll | `FakeLooper::pollAll()` | `fake_looper::poll_all()` (Rust) | Done |
| FakeLooper attachInputQueue | `FakeLooper::attachInputQueue()` | `fake_looper::attach_input_queue()` (Rust) | Done |
| hybris hook lambdas | 6 lambdas in jni_bridge_stub.cpp | `fake_looper.rs` hook registration | Done |
| start_game_with_baron | `JniSupport::startGame()` (Baron path) | `jni_support::jni_support_start_game_with_baron()` (Rust) | Done |

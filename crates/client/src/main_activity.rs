use libjnivm_sys::*;
use std::ffi::{c_char, CStr, CString};
use std::sync::OnceLock;

const JNI_TRUE: jboolean = 1;
const JNI_FALSE: jboolean = 0;

fn get_iface(env: *mut JNIEnv) -> *mut JNINativeInterface {
    if env.is_null() {
        return std::ptr::null_mut();
    }
    unsafe { *(env as *mut *mut JNINativeInterface) }
}

extern "C" {
    fn jnivm_get_main_window() -> *mut std::ffi::c_void;
    fn jnivm_get_storage_dir() -> *const c_char;
    fn jnivm_get_text_input_handler() -> *mut std::ffi::c_void;
    fn jnivm_get_asset_manager() -> *mut std::ffi::c_void;
    fn jnivm_get_stbi_load_from_memory() -> *mut std::ffi::c_void;
    fn jnivm_get_stbi_image_free() -> *mut std::ffi::c_void;
    fn core_patches_hide_mouse_pointer();
    fn core_patches_show_mouse_pointer();
    fn eglutSetClipboardText(text: *const c_char);
}

fn storage_dir() -> &'static str {
    static DIR: OnceLock<String> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = unsafe { jnivm_get_storage_dir() };
        if dir.is_null() {
            "/tmp".to_string()
        } else {
            unsafe { std::ffi::CStr::from_ptr(dir) }.to_string_lossy().into_owned()
        }
    })
}

#[repr(C)]
struct FileObject {
    path: [i8; 4096],
}

fn create_file_object(env: *mut JNIEnv, path: &str) -> jobject {
    let len = path.len().min(4095);
    let mut fobj = Box::new(FileObject { path: [0i8; 4096] });
    let src = path.as_bytes();
    for (i, &b) in src[..len].iter().enumerate() {
        fobj.path[i] = b as i8;
    }
    Box::into_raw(fobj) as jobject
}

// ========== Phase 0: Trivial stubs ==========

unsafe extern "C" fn get_android_version(_env: *mut JNIEnv, _self: jobject) -> jint {
    32
}

unsafe extern "C" fn get_screen_width(_env: *mut JNIEnv, _self: jobject) -> jint {
    1600
}

unsafe extern "C" fn get_screen_height(_env: *mut JNIEnv, _self: jobject) -> jint {
    1200
}

unsafe extern "C" fn get_display_width(env: *mut JNIEnv, self_: jobject) -> jint {
    get_screen_width(env, self_)
}

unsafe extern "C" fn get_display_height(env: *mut JNIEnv, self_: jobject) -> jint {
    get_screen_height(env, self_)
}

unsafe extern "C" fn tick(_env: *mut JNIEnv, _self: jobject) {}

unsafe extern "C" fn is_network_enabled(
    _env: *mut JNIEnv,
    _self: jobject,
    _wifi: jboolean,
) -> jboolean {
    JNI_TRUE
}

unsafe extern "C" fn is_chromebook(_env: *mut JNIEnv, _self: jobject) -> jboolean {
    JNI_TRUE
}

unsafe extern "C" fn get_device_model(env: *mut JNIEnv, _self: jobject) -> jstring {
    let iface = get_iface(env);
    if iface.is_null() {
        return std::ptr::null_mut();
    }
    let new_string = match (*iface).NewStringUTF {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    new_string(env, b"Linux\0".as_ptr() as *const c_char) as jstring
}

unsafe extern "C" fn has_hardware_keyboard(_env: *mut JNIEnv, _self: jobject) -> jboolean {
    JNI_TRUE
}

unsafe extern "C" fn get_cursor_position(_env: *mut JNIEnv, _self: jobject) -> jint {
    0
}

unsafe extern "C" fn get_text_box_backend(env: *mut JNIEnv, _self: jobject) -> jstring {
    let iface = get_iface(env);
    if iface.is_null() {
        return std::ptr::null_mut();
    }
    let new_string = match (*iface).NewStringUTF {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    new_string(env, b"\0".as_ptr() as *const c_char) as jstring
}

unsafe extern "C" fn set_caret_position(_env: *mut JNIEnv, _self: jobject, _pos: jint) {}

unsafe extern "C" fn set_last_char(_env: *mut JNIEnv, _self: jobject, _sym: jint) {}

unsafe extern "C" fn start_play_integrity_check(_env: *mut JNIEnv, _self: jobject) {}

unsafe extern "C" fn get_broadcast_addresses(env: *mut JNIEnv, _self: jobject) -> jobject {
    let iface = get_iface(env);
    if iface.is_null() {
        return std::ptr::null_mut();
    }
    let find_class = match (*iface).FindClass {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let new_array = match (*iface).NewObjectArray {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let string_cls = find_class(env, b"java/lang/String\0".as_ptr() as *const c_char);
    if string_cls.is_null() {
        return std::ptr::null_mut();
    }
    new_array(env, 0, string_cls, std::ptr::null_mut()) as jobject
}

unsafe extern "C" fn update_textbox_text(_env: *mut JNIEnv, _self: jobject, _text: jstring) {}

unsafe extern "C" fn set_text_box_backend(_env: *mut JNIEnv, _self: jobject, _text: jstring) {}

// ========== Phase 1: OS syscalls + file stubs ==========

unsafe extern "C" fn get_used_memory(_env: *mut JNIEnv, _self: jobject) -> jlong {
    let content = std::fs::read_to_string("/proc/self/statm").unwrap_or_default();
    let first = content.split_whitespace().next().unwrap_or("0");
    let pages: i64 = first.parse().unwrap_or(0);
    pages * 4096
}

unsafe extern "C" fn get_free_memory(_env: *mut JNIEnv, _self: jobject) -> jlong {
    let content = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    for line in content.lines() {
        if line.starts_with("MemAvailable:") {
            if let Some(val) = line.split_whitespace().nth(1) {
                if let Ok(kb) = val.parse::<i64>() {
                    return kb * 1024;
                }
            }
        }
    }
    0
}

unsafe extern "C" fn get_total_memory(_env: *mut JNIEnv, _self: jobject) -> jlong {
    let content = std::fs::read_to_string("/proc/meminfo").unwrap_or_default();
    for line in content.lines() {
        if line.starts_with("MemTotal:") {
            if let Some(val) = line.split_whitespace().nth(1) {
                if let Ok(kb) = val.parse::<i64>() {
                    return kb * 1024;
                }
            }
        }
    }
    0
}

unsafe extern "C" fn get_memory_limit(env: *mut JNIEnv, self_: jobject) -> jlong {
    get_total_memory(env, self_)
}

unsafe extern "C" fn get_available_memory(env: *mut JNIEnv, self_: jobject) -> jlong {
    get_free_memory(env, self_)
}

unsafe extern "C" fn get_allocatable_bytes(
    _env: *mut JNIEnv,
    _self: jobject,
    _path: jstring,
) -> jlong {
    1024i64 * 1024 * 1024 * 1024
}

unsafe extern "C" fn supports_size_query(
    _env: *mut JNIEnv,
    _self: jobject,
    _path: jstring,
) -> jboolean {
    JNI_TRUE
}

unsafe extern "C" fn calculate_available_disk_free_space(
    _env: *mut JNIEnv,
    _self: jobject,
    _path: jstring,
) -> jlong {
    1024i64 * 1024 * 1024 * 1024
}

unsafe extern "C" fn get_usable_space(
    _env: *mut JNIEnv,
    _self: jobject,
    _path: jstring,
) -> jlong {
    1024i64 * 1024 * 1024 * 1024
}

unsafe extern "C" fn get_platform_dpi(_env: *mut JNIEnv, _self: jobject) -> jint {
    192
}

unsafe extern "C" fn get_pixels_per_millimeter(
    _env: *mut JNIEnv,
    _self: jobject,
) -> jfloat {
    (96.0f32 / 25.4) * 2.0
}

unsafe extern "C" fn has_write_external_storage_permission(
    _env: *mut JNIEnv,
    _self: jobject,
) -> jboolean {
    JNI_TRUE
}

unsafe extern "C" fn has_read_media_images_permission(
    _env: *mut JNIEnv,
    _self: jobject,
) -> jboolean {
    JNI_TRUE
}

unsafe extern "C" fn get_file_data_bytes(
    _env: *mut JNIEnv,
    _self: jobject,
    _path: jstring,
) -> jlong {
    0
}

// ========== Phase 2: Global-state methods ==========

unsafe extern "C" fn get_files_dir(env: *mut JNIEnv, _self: jobject) -> jobject {
    create_file_object(env, storage_dir())
}

unsafe extern "C" fn get_cache_dir(env: *mut JNIEnv, self_: jobject) -> jobject {
    get_files_dir(env, self_)
}

unsafe extern "C" fn get_external_storage_path(env: *mut JNIEnv, _self: jobject) -> jstring {
    let iface = get_iface(env);
    if iface.is_null() { return std::ptr::null_mut(); }
    let new_string = match (*iface).NewStringUTF {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let dir = CString::new(storage_dir()).unwrap_or_default();
    new_string(env, dir.as_ptr()) as jstring
}

unsafe extern "C" fn get_internal_storage_path(env: *mut JNIEnv, self_: jobject) -> jstring {
    get_external_storage_path(env, self_)
}

unsafe extern "C" fn get_legacy_external_storage_path(env: *mut JNIEnv, _self: jobject, _game_folder: jstring) -> jstring {
    get_external_storage_path(env, _self)
}

unsafe extern "C" fn get_locale(env: *mut JNIEnv, _self: jobject) -> jstring {
    let iface = get_iface(env);
    if iface.is_null() { return std::ptr::null_mut(); }
    let new_string = match (*iface).NewStringUTF {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    new_string(env, b"en\0".as_ptr() as *const c_char) as jstring
}

unsafe extern "C" fn get_hardware_info(_env: *mut JNIEnv, _self: jobject) -> jobject {
    Box::into_raw(Box::new(0u8)) as jobject
}

unsafe extern "C" fn show_keyboard(
    _env: *mut JNIEnv,
    _self: jobject,
    _text: jstring,
    _max_len: jint,
    _ignored1: jboolean,
    _ignored2: jboolean,
    _multiline: jboolean,
) {
}

unsafe extern "C" fn hide_keyboard(_env: *mut JNIEnv, _self: jobject) {}

unsafe extern "C" fn get_ip_addresses(env: *mut JNIEnv, _self: jobject) -> jobject {
    let iface = get_iface(env);
    if iface.is_null() { return std::ptr::null_mut(); }
    let find_class = match (*iface).FindClass {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let new_array = match (*iface).NewObjectArray {
        Some(f) => f,
        None => return std::ptr::null_mut(),
    };
    let string_cls = find_class(env, b"java/lang/String\0".as_ptr() as *const c_char);
    if string_cls.is_null() { return std::ptr::null_mut(); }
    new_array(env, 0, string_cls, std::ptr::null_mut()) as jobject
}

unsafe extern "C" fn get_caret_position(_env: *mut JNIEnv, _self: jobject) -> jint {
    0
}

unsafe extern "C" fn set_clipboard(env: *mut JNIEnv, _self: jobject, text: jstring) {
    if let Some(s) = get_jstring_content(env, text) {
        if let Ok(c_str) = CString::new(s) {
            eglutSetClipboardText(c_str.as_ptr());
        }
    }
}

unsafe extern "C" fn get_key_from_key_code(
    _env: *mut JNIEnv,
    _self: jobject,
    key_code: jint,
    _meta_state: jint,
    _device_id: jint,
) -> jint {
    key_code
}

// ========== Phase 3: Heavy methods ==========

unsafe extern "C" fn create_uuid(env: *mut JNIEnv, _self: jobject) -> jobject {
    let iface = get_iface(env);
    if iface.is_null() { return std::ptr::null_mut(); }
    let find_class = match (*iface).FindClass { Some(f) => f, None => return std::ptr::null_mut() };
    let get_static_mid = match (*iface).GetStaticMethodID { Some(f) => f, None => return std::ptr::null_mut() };
    let call_static = match (*iface).CallStaticObjectMethod { Some(f) => f, None => return std::ptr::null_mut() };

    let uuid_cls = find_class(env, b"java/util/UUID\0".as_ptr() as *const c_char);
    if uuid_cls.is_null() { return std::ptr::null_mut(); }
    let mid = get_static_mid(env, uuid_cls, b"randomUUID\0".as_ptr() as *const c_char, b"()Ljava/util/UUID;\0".as_ptr() as *const c_char);
    if mid.is_null() { return std::ptr::null_mut(); }
    call_static(env, uuid_cls, mid)
}

unsafe extern "C" fn lock_cursor(_env: *mut JNIEnv, _self: jobject) {
    core_patches_hide_mouse_pointer();
}

unsafe extern "C" fn unlock_cursor(_env: *mut JNIEnv, _self: jobject) {
    core_patches_show_mouse_pointer();
}

fn call_jni_void_method(env: *mut JNIEnv, self_ref: jobject, name: &[u8], sig: &[u8], args: &mut [jvalue]) {
    let iface = get_iface(env);
    if iface.is_null() { return; }
    let get_class = match unsafe { (*iface).GetObjectClass } { Some(f) => f, None => return };
    let get_mid = match unsafe { (*iface).GetMethodID } { Some(f) => f, None => return };
    let call = match unsafe { (*iface).CallVoidMethodA } { Some(f) => f, None => return };

    let cls = unsafe { get_class(env, self_ref) };
    if cls.is_null() { return; }
    let mid = unsafe { get_mid(env, cls, name.as_ptr() as *const c_char, sig.as_ptr() as *const c_char) };
    if mid.is_null() { return; }
    unsafe { call(env, self_ref, mid, args.as_mut_ptr()) };
}

fn get_jstring_content(env: *mut JNIEnv, s: jstring) -> Option<String> {
    let iface = get_iface(env);
    if iface.is_null() { return None; }
    let get_chars = match unsafe { (*iface).GetStringUTFChars } { Some(f) => f, None => return None };
    let release = unsafe { (*iface).ReleaseStringUTFChars };
    let c_str = unsafe { get_chars(env, s, std::ptr::null_mut()) };
    if c_str.is_null() { return None; }
    let result = Some(unsafe { std::ffi::CStr::from_ptr(c_str) }.to_string_lossy().into_owned());
    if let Some(f) = release { unsafe { f(env, s, c_str) }; }
    result
}

fn new_jstring(env: *mut JNIEnv, s: &str) -> jstring {
    let iface = get_iface(env);
    if iface.is_null() { return std::ptr::null_mut(); }
    let new_string = match unsafe { (*iface).NewStringUTF } { Some(f) => f, None => return std::ptr::null_mut() };
    let c_str = CString::new(s).unwrap_or_default();
    unsafe { new_string(env, c_str.as_ptr()) as jstring }
}

fn show_file_picker_dialog(title: &str, save: bool, filter: Option<&str>) -> Option<String> {
    let mut cmd = std::process::Command::new("zenity");
    cmd.arg("--file-selection");
    cmd.arg("--title");
    cmd.arg(title);
    if save { cmd.arg("--save"); }
    if let Some(f) = filter {
        cmd.arg("--file-filter");
        cmd.arg(f);
    }
    let output = cmd.output().ok()?;
    if !output.status.success() { return None; }
    let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if path.is_empty() { return None; }
    Some(path)
}

unsafe extern "C" fn pick_image(env: *mut JNIEnv, self_: jobject, callback: jlong) {
    let picked = show_file_picker_dialog("Select image", false, Some("*.png"));
    match picked {
        Some(path) => {
            let jpath = new_jstring(env, &path);
            let mut args = [jvalue { j: callback }, jvalue { l: jpath }];
            call_jni_void_method(env, self_, b"nativeOnPickImageSuccess\0", b"(JLjava/lang/String;)V\0", &mut args);
        }
        None => {
            let mut args = [jvalue { j: callback }];
            call_jni_void_method(env, self_, b"nativeOnPickImageCanceled\0", b"(J)V\0", &mut args);
        }
    }
}

unsafe extern "C" fn open_file(env: *mut JNIEnv, self_: jobject) {
    let picked = show_file_picker_dialog("Select file", false, None);
    match picked {
        Some(path) => {
            let jpath = new_jstring(env, &path);
            let mut args = [jvalue { l: jpath }];
            call_jni_void_method(env, self_, b"nativeOnPickFileSuccess\0", b"(Ljava/lang/String;)V\0", &mut args);
        }
        None => {
            call_jni_void_method(env, self_, b"nativeOnPickFileCanceled\0", b"()V\0", &mut []);
        }
    }
}

unsafe extern "C" fn save_file(env: *mut JNIEnv, self_: jobject, file_name: jstring) {
    let name = get_jstring_content(env, file_name).unwrap_or_default();
    let picked = show_file_picker_dialog("Save file", true, None);
    match picked {
        Some(path) => {
            // Copy the original file to the picked path (matching C++ behavior)
            if !name.is_empty() {
                let _ = std::fs::copy(&name, &path);
            }
            let jpath = new_jstring(env, &path);
            let mut args = [jvalue { l: jpath }];
            call_jni_void_method(env, self_, b"nativeOnPickFileSuccess\0", b"(Ljava/lang/String;)V\0", &mut args);
        }
        None => {
            call_jni_void_method(env, self_, b"nativeOnPickFileCanceled\0", b"()V\0", &mut []);
        }
    }
}

unsafe extern "C" fn launch_uri(_env: *mut JNIEnv, _self: jobject, uri: jstring) {
    if let Some(uri_str) = get_jstring_content(_env, uri) {
        let _ = std::process::Command::new("xdg-open")
            .arg(&uri_str)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn();
    }
}

unsafe extern "C" fn share(_env: *mut JNIEnv, _self: jobject, title: jstring, string: jstring, url: jstring) {
    let title_str = get_jstring_content(_env, title).unwrap_or_default();
    let string_str = get_jstring_content(_env, string).unwrap_or_default();
    let url_str = get_jstring_content(_env, url).unwrap_or_default();
    let text = if url_str.is_empty() { string_str } else { format!("{}\n{}", string_str, url_str) };
    if title_str.contains('"') || text.contains('"') { return; }
    let _ = std::process::Command::new("zenity")
        .arg("--info")
        .arg("--title")
        .arg(&title_str)
        .arg("--text")
        .arg(&text)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn();
}

unsafe extern "C" fn share_file(_env: *mut JNIEnv, _self: jobject, _title: jstring, _string: jstring, _path: jstring) {
    // TODO: implement file copy with picker
}

unsafe extern "C" fn get_image_data(env: *mut JNIEnv, _self: jobject, filename: jstring) -> jobject {
    let stbi_load_ptr = jnivm_get_stbi_load_from_memory();
    let stbi_free_ptr = jnivm_get_stbi_image_free();
    if stbi_load_ptr.is_null() || stbi_free_ptr.is_null() {
        return std::ptr::null_mut();
    }
    let stbi_load: unsafe extern "C" fn(*const u8, i32, *mut i32, *mut i32, *mut i32, i32) -> *mut u8 =
        std::mem::transmute(stbi_load_ptr);
    let stbi_free: unsafe extern "C" fn(*mut u8) = std::mem::transmute(stbi_free_ptr);

    let path = match get_jstring_content(env, filename) {
        Some(p) => p,
        None => return std::ptr::null_mut(),
    };
    let buf = match std::fs::read(&path) {
        Ok(b) => b,
        Err(_) => return std::ptr::null_mut(),
    };
    let mut width: i32 = 0;
    let mut height: i32 = 0;
    let mut channels: i32 = 0;
    let image = stbi_load(buf.as_ptr(), buf.len() as i32, &mut width, &mut height, &mut channels, 4);
    if image.is_null() {
        return std::ptr::null_mut();
    }
    let pixel_count = (width * height) as usize;
    let len = (2 + pixel_count) as jsize;
    let iface = get_iface(env);
    if iface.is_null() { stbi_free(image); return std::ptr::null_mut(); }
    let new_array = match (*iface).NewIntArray { Some(f) => f, None => { stbi_free(image); return std::ptr::null_mut(); } };
    let set_region = match (*iface).SetIntArrayRegion { Some(f) => f, None => { stbi_free(image); return std::ptr::null_mut(); } };
    let arr = new_array(env, len);
    if arr.is_null() { stbi_free(image); return std::ptr::null_mut(); }
    let mut pixels: Vec<jint> = Vec::with_capacity(2 + pixel_count);
    pixels.push(width);
    pixels.push(height);
    for x in 0..pixel_count {
        let r = *image.add(x * 4 + 0) as i32;
        let g = *image.add(x * 4 + 1) as i32;
        let b = *image.add(x * 4 + 2) as i32;
        let a = *image.add(x * 4 + 3) as i32;
        pixels.push(b | (g << 8) | (r << 16) | (a << 24));
    }
    set_region(env, arr, 0, len, pixels.as_ptr());
    stbi_free(image);
    arr as jobject
}

unsafe extern "C" fn run_native_callback_on_ui_thread(_env: *mut JNIEnv, _self: jobject, _handle: jlong) {
    // TODO: call nativeRunNativeCallbackOnUiThread through JNI
}

unsafe extern "C" fn request_integrity_token(env: *mut JNIEnv, self_: jobject, _str: jstring) {
    // Call nativeSetIntegrityToken with a fake UUID token (matching C++ behavior)
    let uuid = create_uuid(env, self_);
    if uuid.is_null() { return; }
    let iface = get_iface(env);
    if iface.is_null() { return; }
    let get_class = match (*iface).GetObjectClass { Some(f) => f, None => return };
    let get_mid = match (*iface).GetMethodID { Some(f) => f, None => return };
    let cls = get_class(env, self_);
    if cls.is_null() { return; }
    let mid = get_mid(env, cls,
        b"nativeSetIntegrityToken\0".as_ptr() as *const c_char,
        b"(Ljava/lang/String;)V\0".as_ptr() as *const c_char);
    if mid.is_null() { return; }
    let uuid_str_mid = get_mid(env, uuid,
        b"toString\0".as_ptr() as *const c_char,
        b"()Ljava/lang/String;\0".as_ptr() as *const c_char);
    if uuid_str_mid.is_null() { return; }
    let call_obj = match (*iface).CallObjectMethod { Some(f) => f, None => return };
    let uuid_str = call_obj(env, uuid, uuid_str_mid);
    if uuid_str.is_null() { return; }
    let call_a = match (*iface).CallVoidMethodA { Some(f) => f, None => return };
    let mut args = [jvalue { l: uuid_str }];
    call_a(env, self_, mid, args.as_mut_ptr());
}

pub fn register(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"getAndroidVersion\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: get_android_version as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getScreenWidth\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: get_screen_width as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getScreenHeight\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: get_screen_height as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getDisplayWidth\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: get_display_width as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getDisplayHeight\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: get_display_height as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"tick\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: tick as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"isNetworkEnabled\0".as_ptr() as *const c_char,
            signature: b"(Z)Z\0".as_ptr() as *const c_char,
            fnPtr: is_network_enabled as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"isChromebook\0".as_ptr() as *const c_char,
            signature: b"()Z\0".as_ptr() as *const c_char,
            fnPtr: is_chromebook as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getDeviceModel\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: get_device_model as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"hasHardwareKeyboard\0".as_ptr() as *const c_char,
            signature: b"()Z\0".as_ptr() as *const c_char,
            fnPtr: has_hardware_keyboard as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getCursorPosition\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: get_cursor_position as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getTextBoxBackend\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: get_text_box_backend as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"setCaretPosition\0".as_ptr() as *const c_char,
            signature: b"(I)V\0".as_ptr() as *const c_char,
            fnPtr: set_caret_position as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"setLastChar\0".as_ptr() as *const c_char,
            signature: b"(I)V\0".as_ptr() as *const c_char,
            fnPtr: set_last_char as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"startPlayIntegrityCheck\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: start_play_integrity_check as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getBroadcastAddresses\0".as_ptr() as *const c_char,
            signature: b"()[Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: get_broadcast_addresses as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"updateTextboxText\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: update_textbox_text as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"setTextBoxBackend\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: set_text_box_backend as *mut std::ffi::c_void,
        },
        // Phase 1: OS syscalls + file stubs
        JNINativeMethod {
            name: b"getUsedMemory\0".as_ptr() as *const c_char,
            signature: b"()J\0".as_ptr() as *const c_char,
            fnPtr: get_used_memory as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getFreeMemory\0".as_ptr() as *const c_char,
            signature: b"()J\0".as_ptr() as *const c_char,
            fnPtr: get_free_memory as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getTotalMemory\0".as_ptr() as *const c_char,
            signature: b"()J\0".as_ptr() as *const c_char,
            fnPtr: get_total_memory as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getMemoryLimit\0".as_ptr() as *const c_char,
            signature: b"()J\0".as_ptr() as *const c_char,
            fnPtr: get_memory_limit as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getAvailableMemory\0".as_ptr() as *const c_char,
            signature: b"()J\0".as_ptr() as *const c_char,
            fnPtr: get_available_memory as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getAllocatableBytes\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)J\0".as_ptr() as *const c_char,
            fnPtr: get_allocatable_bytes as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"supportsSizeQuery\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)Z\0".as_ptr() as *const c_char,
            fnPtr: supports_size_query as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"calculateAvailableDiskFreeSpace\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)J\0".as_ptr() as *const c_char,
            fnPtr: calculate_available_disk_free_space as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getUsableSpace\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)J\0".as_ptr() as *const c_char,
            fnPtr: get_usable_space as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getPlatformDpi\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: get_platform_dpi as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getPixelsPerMillimeter\0".as_ptr() as *const c_char,
            signature: b"()F\0".as_ptr() as *const c_char,
            fnPtr: get_pixels_per_millimeter as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"hasWriteExternalStoragePermission\0".as_ptr() as *const c_char,
            signature: b"()Z\0".as_ptr() as *const c_char,
            fnPtr: has_write_external_storage_permission as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"hasReadMediaImagesPermission\0".as_ptr() as *const c_char,
            signature: b"()Z\0".as_ptr() as *const c_char,
            fnPtr: has_read_media_images_permission as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getFileDataBytes\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)[B\0".as_ptr() as *const c_char,
            fnPtr: get_file_data_bytes as *mut std::ffi::c_void,
        },
        // Phase 2: Global-state methods
        JNINativeMethod {
            name: b"getFilesDir\0".as_ptr() as *const c_char,
            signature: b"()Ljava/io/File;\0".as_ptr() as *const c_char,
            fnPtr: get_files_dir as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getCacheDir\0".as_ptr() as *const c_char,
            signature: b"()Ljava/io/File;\0".as_ptr() as *const c_char,
            fnPtr: get_cache_dir as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getExternalStoragePath\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: get_external_storage_path as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getInternalStoragePath\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: get_internal_storage_path as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getLegacyExternalStoragePath\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: get_legacy_external_storage_path as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getLocale\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: get_locale as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getHardwareInfo\0".as_ptr() as *const c_char,
            signature: b"()Lcom/mojang/minecraftpe/HardwareInformation;\0".as_ptr() as *const c_char,
            fnPtr: get_hardware_info as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"showKeyboard\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;IZZZ)V\0".as_ptr() as *const c_char,
            fnPtr: show_keyboard as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"hideKeyboard\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: hide_keyboard as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getIPAddresses\0".as_ptr() as *const c_char,
            signature: b"()[Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: get_ip_addresses as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getCaretPosition\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: get_caret_position as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"setClipboard\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: set_clipboard as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getKeyFromKeyCode\0".as_ptr() as *const c_char,
            signature: b"(III)I\0".as_ptr() as *const c_char,
            fnPtr: get_key_from_key_code as *mut std::ffi::c_void,
        },
        // Phase 3: Heavy methods
        JNINativeMethod {
            name: b"createUUID\0".as_ptr() as *const c_char,
            signature: b"()Ljava/util/UUID;\0".as_ptr() as *const c_char,
            fnPtr: create_uuid as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"lockCursor\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: lock_cursor as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"unlockCursor\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: unlock_cursor as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"pickImage\0".as_ptr() as *const c_char,
            signature: b"(J)V\0".as_ptr() as *const c_char,
            fnPtr: pick_image as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"openFile\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: open_file as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"saveFile\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: save_file as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"launchUri\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: launch_uri as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"share\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: share as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"shareFile\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: share_file as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getImageData\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)[I\0".as_ptr() as *const c_char,
            fnPtr: get_image_data as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"runNativeCallbackOnUiThread\0".as_ptr() as *const c_char,
            signature: b"(J)V\0".as_ptr() as *const c_char,
            fnPtr: run_native_callback_on_ui_thread as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"requestIntegrityToken\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;)V\0".as_ptr() as *const c_char,
            fnPtr: request_integrity_token as *mut std::ffi::c_void,
        },
    ];

    let cls = unsafe {
        jnivm_find_class(
            env,
            b"com/mojang/minecraftpe/MainActivity\0".as_ptr() as *const c_char,
        )
    };
    if cls.is_null() {
        log::error!(
            "main_activity: FindClass failed for com/mojang/minecraftpe/MainActivity"
        );
        return;
    }
    let rc = unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint) };
    if rc != 0 {
        log::error!("main_activity: RegisterNatives failed (rc={})", rc);
    } else {
        log::info!(
            "main_activity: registered {} methods for com/mojang/minecraftpe/MainActivity",
            methods.len()
        );
    }
}

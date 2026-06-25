use std::ffi::{c_char, c_void, CStr, CString};
use std::sync::Mutex;

struct SendPtr(*mut c_void);
unsafe impl Send for SendPtr {}

static G_MAIN_WINDOW: Mutex<SendPtr> = Mutex::new(SendPtr(std::ptr::null_mut()));
static G_STORAGE_DIR: Mutex<Option<CString>> = Mutex::new(None);
static G_TEXT_INPUT_HANDLER: Mutex<SendPtr> = Mutex::new(SendPtr(std::ptr::null_mut()));
static G_ASSET_MANAGER: Mutex<SendPtr> = Mutex::new(SendPtr(std::ptr::null_mut()));
static G_STBI_LOAD: Mutex<SendPtr> = Mutex::new(SendPtr(std::ptr::null_mut()));
static G_STBI_FREE: Mutex<SendPtr> = Mutex::new(SendPtr(std::ptr::null_mut()));

#[no_mangle]
pub unsafe extern "C" fn jnivm_set_main_window(window: *mut c_void) {
    *G_MAIN_WINDOW.lock().unwrap() = SendPtr(window);
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_main_window() -> *mut c_void {
    G_MAIN_WINDOW.lock().unwrap().0
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_set_storage_dir(dir: *const c_char) {
    if !dir.is_null() {
        let cstr = CStr::from_ptr(dir).to_owned();
        *G_STORAGE_DIR.lock().unwrap() = Some(cstr);
    }
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_storage_dir() -> *const c_char {
    let guard = G_STORAGE_DIR.lock().unwrap();
    guard.as_ref().map(|s| s.as_ptr()).unwrap_or(std::ptr::null())
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_set_text_input_handler(handler: *mut c_void) {
    *G_TEXT_INPUT_HANDLER.lock().unwrap() = SendPtr(handler);
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_text_input_handler() -> *mut c_void {
    G_TEXT_INPUT_HANDLER.lock().unwrap().0
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_set_asset_manager(mgr: *mut c_void) {
    *G_ASSET_MANAGER.lock().unwrap() = SendPtr(mgr);
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_asset_manager() -> *mut c_void {
    G_ASSET_MANAGER.lock().unwrap().0
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_set_stbi_load_from_memory(fn_ptr: *mut c_void) {
    *G_STBI_LOAD.lock().unwrap() = SendPtr(fn_ptr);
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_stbi_load_from_memory() -> *mut c_void {
    G_STBI_LOAD.lock().unwrap().0
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_set_stbi_image_free(fn_ptr: *mut c_void) {
    *G_STBI_FREE.lock().unwrap() = SendPtr(fn_ptr);
}

#[no_mangle]
pub unsafe extern "C" fn jnivm_get_stbi_image_free() -> *mut c_void {
    G_STBI_FREE.lock().unwrap().0
}

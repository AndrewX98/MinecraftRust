use libjnivm_sys::*;
use std::ffi::c_char;
use std::mem;

const JNI_TRUE: jboolean = 1;
const JNI_FALSE: jboolean = 0;

extern "C" {
    fn jnivm_get_storage_dir() -> *const c_char;
}

fn get_iface(env: *mut JNIEnv) -> *mut JNINativeInterface {
    if env.is_null() { return std::ptr::null_mut(); }
    unsafe { *(env as *mut *mut JNINativeInterface) }
}

fn new_jstring(env: *mut JNIEnv, s: &str) -> jstring {
    let iface = get_iface(env);
    if iface.is_null() { return std::ptr::null_mut(); }
    let new_string = match unsafe { (*iface).NewStringUTF } { Some(f) => f, None => return std::ptr::null_mut() };
    let c_str = std::ffi::CString::new(s).unwrap_or_default();
    unsafe { new_string(env, c_str.as_ptr()) as jstring }
}

// ================================================================
// java/io/File
// ================================================================

#[repr(C)]
struct FileObject {
    path: [i8; 4096],
}

unsafe extern "C" fn File_getPath(env: *mut JNIEnv, self_: jobject) -> jstring {
    let f = self_ as *const FileObject;
    let path = std::ffi::CStr::from_ptr((*f).path.as_ptr());
    new_jstring(env, &path.to_string_lossy())
}

unsafe extern "C" fn File_getAbsolutePath(env: *mut JNIEnv, self_: jobject) -> jstring {
    let f = self_ as *const FileObject;
    let path_c = (*f).path.as_ptr();
    let abs = libc::realpath(path_c, std::ptr::null_mut());
    if abs.is_null() {
        return File_getPath(env, self_);
    }
    let s = std::ffi::CStr::from_ptr(abs);
    let result = new_jstring(env, &s.to_string_lossy());
    libc::free(abs as *mut libc::c_void);
    result
}

unsafe extern "C" fn File_exists(_env: *mut JNIEnv, self_: jobject) -> jboolean {
    let f = self_ as *const FileObject;
    let path_c = (*f).path.as_ptr();
    let fp = libc::fopen(path_c, b"r\0".as_ptr() as *const i8);
    if !fp.is_null() {
        libc::fclose(fp);
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

unsafe extern "C" fn File_length(_env: *mut JNIEnv, self_: jobject) -> jlong {
    let f = self_ as *const FileObject;
    let path_c = (*f).path.as_ptr();
    let fp = libc::fopen(path_c, b"rb\0".as_ptr() as *const i8);
    if fp.is_null() { return 0; }
    libc::fseek(fp, 0, libc::SEEK_END);
    let len = libc::ftell(fp);
    libc::fclose(fp);
    len as jlong
}

unsafe extern "C" fn File_isDirectory(_env: *mut JNIEnv, self_: jobject) -> jboolean {
    let f = self_ as *const FileObject;
    let path_c = (*f).path.as_ptr();
    let mut st: libc::stat = mem::zeroed();
    if libc::stat(path_c, &mut st) == 0 && (st.st_mode & libc::S_IFMT) == libc::S_IFDIR {
        JNI_TRUE
    } else {
        JNI_FALSE
    }
}

fn statvfs_space(path: *const i8, field_fn: fn(&libc::statvfs) -> u64) -> jlong {
    let mut stat: libc::statvfs = unsafe { mem::zeroed() };
    let path_str = unsafe { std::ffi::CStr::from_ptr(path).to_string_lossy().into_owned() };
    if unsafe { libc::statvfs(path, &mut stat) } == 0 {
        let val = (field_fn(&stat) * stat.f_bsize as u64) as jlong;
        log::info!("File: statvfs_space({}) -> {}", path_str, val);
        val
    } else {
        log::warn!("File: statvfs_space({}) failed, returning 1TB fallback", path_str);
        1024i64 * 1024 * 1024 * 1024
    }
}

unsafe extern "C" fn File_getTotalSpace(_env: *mut JNIEnv, self_: jobject) -> jlong {
    let f = self_ as *const FileObject;
    statvfs_space((*f).path.as_ptr(), |s| s.f_blocks)
}

unsafe extern "C" fn File_getUsableSpace(_env: *mut JNIEnv, self_: jobject) -> jlong {
    let f = self_ as *const FileObject;
    statvfs_space((*f).path.as_ptr(), |s| s.f_bavail)
}

unsafe extern "C" fn File_getFreeSpace(_env: *mut JNIEnv, self_: jobject) -> jlong {
    let f = self_ as *const FileObject;
    statvfs_space((*f).path.as_ptr(), |s| s.f_bfree)
}

fn register_file_class(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"getPath\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: File_getPath as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getAbsolutePath\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: File_getAbsolutePath as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"exists\0".as_ptr() as *const c_char,
            signature: b"()Z\0".as_ptr() as *const c_char,
            fnPtr: File_exists as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"length\0".as_ptr() as *const c_char,
            signature: b"()J\0".as_ptr() as *const c_char,
            fnPtr: File_length as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"isDirectory\0".as_ptr() as *const c_char,
            signature: b"()Z\0".as_ptr() as *const c_char,
            fnPtr: File_isDirectory as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getTotalSpace\0".as_ptr() as *const c_char,
            signature: b"()J\0".as_ptr() as *const c_char,
            fnPtr: File_getTotalSpace as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getUsableSpace\0".as_ptr() as *const c_char,
            signature: b"()J\0".as_ptr() as *const c_char,
            fnPtr: File_getUsableSpace as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getFreeSpace\0".as_ptr() as *const c_char,
            signature: b"()J\0".as_ptr() as *const c_char,
            fnPtr: File_getFreeSpace as *mut std::ffi::c_void,
        },
    ];
    let cls = unsafe { jnivm_find_class(env, b"java/io/File\0".as_ptr() as *const c_char) };
    if cls.is_null() { return; }
    unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint); }
}

// ================================================================
// android/os/Build$VERSION
// ================================================================

unsafe extern "C" fn BuildVersion_getSdkInt(_env: *mut JNIEnv, _clazz: jclass) -> jint {
    32
}

unsafe extern "C" fn BuildVersion_getRelease(env: *mut JNIEnv, _clazz: jclass) -> jstring {
    new_jstring(env, "AndroidX")
}

fn register_build_version_class(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"getSdkInt\0".as_ptr() as *const c_char,
            signature: b"()I\0".as_ptr() as *const c_char,
            fnPtr: BuildVersion_getSdkInt as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getRelease\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: BuildVersion_getRelease as *mut std::ffi::c_void,
        },
    ];
    let cls = unsafe { jnivm_find_class(env, b"android/os/Build$VERSION\0".as_ptr() as *const c_char) };
    if cls.is_null() { return; }
    unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint); }
}

// ================================================================
// android/content/pm/PackageInfo
// ================================================================

#[repr(C)]
struct PackageInfoObject {
    version_name: [i8; 256],
}

unsafe extern "C" fn PackageInfo_getVersionName(env: *mut JNIEnv, self_: jobject) -> jstring {
    let p = self_ as *const PackageInfoObject;
    let s = std::ffi::CStr::from_ptr((*p).version_name.as_ptr());
    new_jstring(env, &s.to_string_lossy())
}

fn register_package_info_class(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"getVersionName\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: PackageInfo_getVersionName as *mut std::ffi::c_void,
        },
    ];
    let cls = unsafe { jnivm_find_class(env, b"android/content/pm/PackageInfo\0".as_ptr() as *const c_char) };
    if cls.is_null() { return; }
    unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint); }
}

// ================================================================
// android/content/pm/PackageManager
// ================================================================

struct PackageManagerObject {
    _dummy: u8,
}

unsafe extern "C" fn PackageManager_getPackageInfo(_env: *mut JNIEnv, _self_: jobject, _package_name: jstring, _flags: jint) -> jobject {
    let mut version_name = [0i8; 256];
    let src = b"1.0.0\0";
    for (i, &b) in src.iter().enumerate() {
        version_name[i] = b as i8;
    }
    let p = Box::new(PackageInfoObject { version_name });
    Box::into_raw(p) as jobject
}

fn register_package_manager_class(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"getPackageInfo\0".as_ptr() as *const c_char,
            signature: b"(Ljava/lang/String;I)Landroid/content/pm/PackageInfo;\0".as_ptr() as *const c_char,
            fnPtr: PackageManager_getPackageInfo as *mut std::ffi::c_void,
        },
    ];
    let cls = unsafe { jnivm_find_class(env, b"android/content/pm/PackageManager\0".as_ptr() as *const c_char) };
    if cls.is_null() { return; }
    unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint); }
}

// ================================================================
// android/content/Context
// ================================================================

unsafe fn context_make_file(env: *mut JNIEnv) -> jobject {
    let dir = jnivm_get_storage_dir();
    if dir.is_null() { return std::ptr::null_mut(); }
    let path = std::ffi::CStr::from_ptr(dir);
    let len = path.to_bytes().len().min(4095);
    let mut fobj = Box::new(FileObject { path: [0i8; 4096] });
    for (i, &b) in path.to_bytes()[..len].iter().enumerate() {
        fobj.path[i] = b as i8;
    }
    Box::into_raw(fobj) as jobject
}

unsafe extern "C" fn Context_getFilesDir(env: *mut JNIEnv, _self_: jobject) -> jobject {
    context_make_file(env)
}

unsafe extern "C" fn Context_getCacheDir(env: *mut JNIEnv, self_: jobject) -> jobject {
    Context_getFilesDir(env, self_)
}

unsafe extern "C" fn Context_getClassLoader(_env: *mut JNIEnv, _self_: jobject) -> jobject {
    std::ptr::null_mut()
}

unsafe extern "C" fn Context_getApplicationContext(_env: *mut JNIEnv, self_: jobject) -> jobject {
    self_
}

unsafe extern "C" fn Context_getPackageName(env: *mut JNIEnv, _self_: jobject) -> jstring {
    new_jstring(env, "com.mojang.minecraftpe")
}

unsafe extern "C" fn Context_getPackageManager(env: *mut JNIEnv, _self_: jobject) -> jobject {
    let pm = Box::new(PackageManagerObject { _dummy: 0 });
    Box::into_raw(pm) as jobject
}

fn register_context_class(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"getFilesDir\0".as_ptr() as *const c_char,
            signature: b"()Ljava/io/File;\0".as_ptr() as *const c_char,
            fnPtr: Context_getFilesDir as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getCacheDir\0".as_ptr() as *const c_char,
            signature: b"()Ljava/io/File;\0".as_ptr() as *const c_char,
            fnPtr: Context_getCacheDir as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getClassLoader\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/ClassLoader;\0".as_ptr() as *const c_char,
            fnPtr: Context_getClassLoader as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getApplicationContext\0".as_ptr() as *const c_char,
            signature: b"()Landroid/content/Context;\0".as_ptr() as *const c_char,
            fnPtr: Context_getApplicationContext as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getPackageName\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: Context_getPackageName as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getPackageManager\0".as_ptr() as *const c_char,
            signature: b"()Landroid/content/pm/PackageManager;\0".as_ptr() as *const c_char,
            fnPtr: Context_getPackageManager as *mut std::ffi::c_void,
        },
    ];
    let cls = unsafe { jnivm_find_class(env, b"android/content/Context\0".as_ptr() as *const c_char) };
    if cls.is_null() { return; }
    unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint); }
}

// ================================================================
// com/mojang/minecraftpe/HardwareInformation
// ================================================================

unsafe extern "C" fn HardwareInfo_getAndroidVersion(env: *mut JNIEnv, _clazz: jclass) -> jstring {
    new_jstring(env, "Linux")
}

unsafe extern "C" fn HardwareInfo_getInstallerPackageName(env: *mut JNIEnv, _self_: jobject) -> jstring {
    new_jstring(env, "com.mojang.minecraftpe")
}

fn register_hardware_info_class(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"getAndroidVersion\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: HardwareInfo_getAndroidVersion as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"getInstallerPackageName\0".as_ptr() as *const c_char,
            signature: b"()Ljava/lang/String;\0".as_ptr() as *const c_char,
            fnPtr: HardwareInfo_getInstallerPackageName as *mut std::ffi::c_void,
        },
    ];
    let cls = unsafe { jnivm_find_class(env, b"com/mojang/minecraftpe/HardwareInformation\0".as_ptr() as *const c_char) };
    if cls.is_null() { return; }
    unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint); }
}

// ================================================================
// com/mojang/minecraftpe/NetworkMonitor
// ================================================================

unsafe extern "C" fn NetworkMonitor_nativeUpdateNetworkStatus(
    _env: *mut JNIEnv, _self_: jobject, _wifi: jboolean, _mobile: jboolean, _ethernet: jboolean,
) {
}

fn register_network_monitor_class(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"nativeUpdateNetworkStatus\0".as_ptr() as *const c_char,
            signature: b"(ZZZ)V\0".as_ptr() as *const c_char,
            fnPtr: NetworkMonitor_nativeUpdateNetworkStatus as *mut std::ffi::c_void,
        },
    ];
    let cls = unsafe { jnivm_find_class(env, b"com/mojang/minecraftpe/NetworkMonitor\0".as_ptr() as *const c_char) };
    if cls.is_null() { return; }
    unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint); }
}

// ================================================================
// com/mojang/minecraftpe/input/JellyBeanDeviceManager
// ================================================================

unsafe extern "C" fn JellyBeanDeviceManager_onInputDeviceAddedNative(
    _env: *mut JNIEnv, _self_: jobject, _dev_id: jint,
) {
}

unsafe extern "C" fn JellyBeanDeviceManager_onInputDeviceRemovedNative(
    _env: *mut JNIEnv, _self_: jobject, _dev_id: jint,
) {
}

fn register_jelly_bean_device_manager_class(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"onInputDeviceAddedNative\0".as_ptr() as *const c_char,
            signature: b"(I)V\0".as_ptr() as *const c_char,
            fnPtr: JellyBeanDeviceManager_onInputDeviceAddedNative as *mut std::ffi::c_void,
        },
        JNINativeMethod {
            name: b"onInputDeviceRemovedNative\0".as_ptr() as *const c_char,
            signature: b"(I)V\0".as_ptr() as *const c_char,
            fnPtr: JellyBeanDeviceManager_onInputDeviceRemovedNative as *mut std::ffi::c_void,
        },
    ];
    let cls = unsafe { jnivm_find_class(env, b"com/mojang/minecraftpe/input/JellyBeanDeviceManager\0".as_ptr() as *const c_char) };
    if cls.is_null() { return; }
    unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint); }
}

// ================================================================
// com/mojang/minecraftpe/PlayIntegrity
// ================================================================

unsafe extern "C" fn PlayIntegrity_nativePlayIntegrityComplete(
    _env: *mut JNIEnv, _self_: jobject,
) {
}

fn register_play_integrity_class(env: *mut JNIEnv) {
    let methods = [
        JNINativeMethod {
            name: b"nativePlayIntegrityComplete\0".as_ptr() as *const c_char,
            signature: b"()V\0".as_ptr() as *const c_char,
            fnPtr: PlayIntegrity_nativePlayIntegrityComplete as *mut std::ffi::c_void,
        },
    ];
    let cls = unsafe { jnivm_find_class(env, b"com/mojang/minecraftpe/PlayIntegrity\0".as_ptr() as *const c_char) };
    if cls.is_null() { return; }
    unsafe { jnivm_register_natives(env, cls, methods.as_ptr(), methods.len() as jint); }
}

// ================================================================
// Main registration entry point
// ================================================================

pub fn register_all(env: *mut JNIEnv) {
    register_file_class(env);
    register_build_version_class(env);
    register_package_info_class(env);
    register_package_manager_class(env);
    register_context_class(env);
    register_hardware_info_class(env);
    register_network_monitor_class(env);
    register_jelly_bean_device_manager_class(env);
    register_play_integrity_class(env);
}

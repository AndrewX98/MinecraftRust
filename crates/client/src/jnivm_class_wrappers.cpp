/// C++ extern "C" wrappers that register Java class methods with libjnivm-sys.
///
/// Each Java class gets a set of extern "C" function implementations that
/// use standard JNI API (not FakeJni). These are registered with libjnivm-sys
/// via FindClass + RegisterNatives, so the game can use the pure Rust JNI VM.
///
/// Rust code calls register_all_jnivm_classes() (declared in jni_support.rs)
/// before the game starts, which iterates all registered classes.

#include <jni.h>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <string>
#include <vector>
#include <unordered_map>
#include <fstream>
#include <sys/statvfs.h>

// Forward declare the libjnivm-sys extern "C" functions
extern "C" {
    void* jnivm_get_env(void* vm);
    void* jnivm_find_class(void* env, const char* name);
    int jnivm_register_natives(void* env, void* clazz, const JNINativeMethod* methods, int count);
}

// ================================================================
// Helper: register a class with its methods in libjnivm-sys
// ================================================================

static void register_class(
    JNIEnv* env,
    const char* class_name,
    const JNINativeMethod* methods,
    int method_count
) {
    jclass cls = env->FindClass(class_name);
    if (cls == nullptr) {
        fprintf(stderr, "[jnivm_wrapper] FindClass failed: %s\n", class_name);
        return;
    }
    int rc = env->RegisterNatives(cls, methods, method_count);
    if (rc != 0) {
        fprintf(stderr, "[jnivm_wrapper] RegisterNatives failed: %s (rc=%d)\n", class_name, rc);
    } else {
        fprintf(stderr, "[jnivm_wrapper] Registered %s (%d methods)\n", class_name, method_count);
    }
}

// ================================================================
// java/io/File
// ================================================================

struct FileObject {
    char path[4096];
};

static FileObject* file_from_jobject(jobject obj) {
    return (FileObject*)obj;
}

extern "C" jobject JNICALL File_getPath(JNIEnv* env, jobject self) {
    auto* f = file_from_jobject(self);
    return env->NewStringUTF(f->path);
}

extern "C" jboolean JNICALL File_exists(JNIEnv* env, jobject self) {
    auto* f = file_from_jobject(self);
    FILE* fp = fopen(f->path, "r");
    if (fp) { fclose(fp); return JNI_TRUE; }
    return JNI_FALSE;
}

extern "C" jlong JNICALL File_length(JNIEnv* env, jobject self) {
    auto* f = file_from_jobject(self);
    FILE* fp = fopen(f->path, "rb");
    if (!fp) return 0;
    fseek(fp, 0, SEEK_END);
    long len = ftell(fp);
    fclose(fp);
    return len;
}

extern "C" jboolean JNICALL File_isDirectory(JNIEnv* env, jobject self) {
    auto* f = file_from_jobject(self);
    FILE* fp = fopen(f->path, "r");
    if (fp) { fclose(fp); return JNI_FALSE; }
    // Try as directory
    std::string cmd = std::string("test -d \"") + f->path + "\"";
    return system(cmd.c_str()) == 0 ? JNI_TRUE : JNI_FALSE;
}

extern "C" jstring JNICALL File_getAbsolutePath(JNIEnv* env, jobject self) {
    auto* f = file_from_jobject(self);
    char* abs = realpath(f->path, nullptr);
    if (abs) {
        jstring ret = env->NewStringUTF(abs);
        free(abs);
        return ret;
    }
    return env->NewStringUTF(f->path);
}

extern "C" jlong JNICALL File_getTotalSpace(JNIEnv* env, jobject self) {
    auto* f = file_from_jobject(self);
    struct statvfs stat;
    if (::statvfs(f->path, &stat) == 0) {
        return (jlong)stat.f_blocks * stat.f_bsize;
    }
    return 1024LL * 1024LL * 1024LL * 1024LL;
}

extern "C" jlong JNICALL File_getUsableSpace(JNIEnv* env, jobject self) {
    auto* f = file_from_jobject(self);
    struct statvfs stat;
    if (::statvfs(f->path, &stat) == 0) {
        return (jlong)stat.f_bavail * stat.f_bsize;
    }
    return 1024LL * 1024LL * 1024LL * 1024LL;
}

extern "C" jlong JNICALL File_getFreeSpace(JNIEnv* env, jobject self) {
    auto* f = file_from_jobject(self);
    struct statvfs stat;
    if (::statvfs(f->path, &stat) == 0) {
        return (jlong)stat.f_bfree * stat.f_bsize;
    }
    return 1024LL * 1024LL * 1024LL * 1024LL;
}

static void register_file_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"getPath",             (char*)"()Ljava/lang/String;",  (void*)&File_getPath},
        {(char*)"getAbsolutePath",     (char*)"()Ljava/lang/String;",  (void*)&File_getAbsolutePath},
        {(char*)"exists",              (char*)"()Z",                   (void*)&File_exists},
        {(char*)"length",              (char*)"()J",                   (void*)&File_length},
        {(char*)"isDirectory",         (char*)"()Z",                   (void*)&File_isDirectory},
        {(char*)"getTotalSpace",       (char*)"()J",                   (void*)&File_getTotalSpace},
        {(char*)"getUsableSpace",      (char*)"()J",                   (void*)&File_getUsableSpace},
        {(char*)"getFreeSpace",        (char*)"()J",                   (void*)&File_getFreeSpace},
    };
    register_class(env, "java/io/File", methods, 8);
}

// ================================================================
// android/os/Build$VERSION
// ================================================================

extern "C" jint JNICALL BuildVersion_getSdkInt(JNIEnv* env, jclass clazz) {
    return 32; // Android 12L
}

extern "C" jstring JNICALL BuildVersion_getRelease(JNIEnv* env, jclass clazz) {
    return env->NewStringUTF("AndroidX");
}

static void register_build_version_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"getSdkInt", (char*)"()I", (void*)&BuildVersion_getSdkInt},
        {(char*)"getRelease", (char*)"()Ljava/lang/String;", (void*)&BuildVersion_getRelease},
    };
    // NOTE: class name has $, which JNI uses /
    register_class(env, "android/os/Build$VERSION", methods, 2);
}

// ================================================================
// android/content/pm/PackageInfo
// ================================================================

struct PackageInfoObject {
    char versionName[256];
};

extern "C" jstring JNICALL PackageInfo_getVersionName(JNIEnv* env, jobject self) {
    auto* p = (PackageInfoObject*)self;
    return env->NewStringUTF(p->versionName);
}

static void register_package_info_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"getVersionName", (char*)"()Ljava/lang/String;", (void*)&PackageInfo_getVersionName},
    };
    register_class(env, "android/content/pm/PackageInfo", methods, 1);
}

// ================================================================
// android/content/pm/PackageManager
// ================================================================

struct PackageManagerObject {
    char dummy;
};

extern "C" jobject JNICALL PackageManager_getPackageInfo(JNIEnv* env, jobject self, jstring packageName, jint flags) {
    // Return a new PackageInfo
    auto* pkg = new PackageInfoObject();
    strcpy(pkg->versionName, "1.0.0");
    return (jobject)pkg;
}

static void register_package_manager_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"getPackageInfo", (char*)"(Ljava/lang/String;I)Landroid/content/pm/PackageInfo;", (void*)&PackageManager_getPackageInfo},
    };
    register_class(env, "android/content/pm/PackageManager", methods, 1);
}

// ================================================================
// android/content/Context
// ================================================================

extern "C" jobject JNICALL Context_getFilesDir(JNIEnv* env, jobject self) {
    // Return a File pointing to the storage directory
    // The storage directory is set externally via context_set_storage_dir
    // For now, return null - JniSupport sets this via reflection
    return nullptr;
}

extern "C" jobject JNICALL Context_getCacheDir(JNIEnv* env, jobject self) {
    return Context_getFilesDir(env, self);
}

extern "C" jobject JNICALL Context_getClassLoader(JNIEnv* env, jobject self) {
    // Return null - ClassLoader is not critical
    return nullptr;
}

extern "C" jobject JNICALL Context_getApplicationContext(JNIEnv* env, jobject self) {
    return self; // itself
}

extern "C" jstring JNICALL Context_getPackageName(JNIEnv* env, jobject self) {
    return env->NewStringUTF("com.mojang.minecraftpe");
}

extern "C" jobject JNICALL Context_getPackageManager(JNIEnv* env, jobject self) {
    auto* pm = new PackageManagerObject();
    return (jobject)pm;
}

static void register_context_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"getFilesDir",             (char*)"()Ljava/io/File;",                  (void*)&Context_getFilesDir},
        {(char*)"getCacheDir",             (char*)"()Ljava/io/File;",                  (void*)&Context_getCacheDir},
        {(char*)"getClassLoader",          (char*)"()Ljava/lang/ClassLoader;",         (void*)&Context_getClassLoader},
        {(char*)"getApplicationContext",   (char*)"()Landroid/content/Context;",       (void*)&Context_getApplicationContext},
        {(char*)"getPackageName",          (char*)"()Ljava/lang/String;",              (void*)&Context_getPackageName},
        {(char*)"getPackageManager",       (char*)"()Landroid/content/pm/PackageManager;", (void*)&Context_getPackageManager},
    };
    register_class(env, "android/content/Context", methods, 6);
}

// ================================================================
// com/mojang/minecraftpe/HardwareInformation
// ================================================================

extern "C" jstring JNICALL HardwareInfo_getAndroidVersion(JNIEnv* env, jclass clazz) {
    return env->NewStringUTF("Linux");
}

extern "C" jstring JNICALL HardwareInfo_getInstallerPackageName(JNIEnv* env, jobject self) {
    return env->NewStringUTF("com.mojang.minecraftpe");
}

static void register_hardware_info_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"getAndroidVersion",       (char*)"()Ljava/lang/String;",  (void*)&HardwareInfo_getAndroidVersion},
        {(char*)"getInstallerPackageName", (char*)"()Ljava/lang/String;",  (void*)&HardwareInfo_getInstallerPackageName},
    };
    register_class(env, "com/mojang/minecraftpe/HardwareInformation", methods, 2);
}

// ================================================================
// com/mojang/minecraftpe/NetworkMonitor
// ================================================================

extern "C" void JNICALL NetworkMonitor_nativeUpdateNetworkStatus(JNIEnv* env, jobject self, jboolean wifi, jboolean mobile, jboolean ethernet) {
    // Native method - resolved from game library. This is just a placeholder.
    fprintf(stderr, "[NetworkMonitor] nativeUpdateNetworkStatus(%d,%d,%d)\n", wifi, mobile, ethernet);
}

static void register_network_monitor_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"nativeUpdateNetworkStatus", (char*)"(ZZZ)V", (void*)&NetworkMonitor_nativeUpdateNetworkStatus},
    };
    register_class(env, "com/mojang/minecraftpe/NetworkMonitor", methods, 1);
}

// ================================================================
// com/mojang/minecraftpe/MainActivity
// ================================================================

// Global state shared by MainActivity (window, storage dir, text input)
// These are set by JniSupport before the game starts.
static void* g_main_window = nullptr;
static char g_storage_dir[4096] = "/tmp";

static void* g_asset_manager = nullptr;

extern "C" void jnivm_set_main_window(void* window) { g_main_window = window; }
extern "C" void jnivm_set_storage_dir(const char* dir) {
    if (dir) { strncpy(g_storage_dir, dir, sizeof(g_storage_dir) - 1); }
}
extern "C" void jnivm_set_asset_manager(void* mgr) { g_asset_manager = mgr; }

// stbi image loading function pointers — set by Rust jni_support_start_game()
static void* g_stbi_load_from_memory = nullptr;
static void* g_stbi_image_free = nullptr;

extern "C" void jnivm_set_stbi_load_from_memory(void* fn) { g_stbi_load_from_memory = fn; }
extern "C" void jnivm_set_stbi_image_free(void* fn) { g_stbi_image_free = fn; }

// Getters for Rust main_activity module
extern "C" void* jnivm_get_main_window() { return g_main_window; }
extern "C" const char* jnivm_get_storage_dir() { return g_storage_dir; }
extern "C" void* jnivm_get_asset_manager() { return g_asset_manager; }
extern "C" void* jnivm_get_stbi_load_from_memory() { return g_stbi_load_from_memory; }
extern "C" void* jnivm_get_stbi_image_free() { return g_stbi_image_free; }

extern "C" jint JNICALL MainActivity_getAndroidVersion(JNIEnv* env, jobject self) {
    return 32;
}

extern "C" jint JNICALL MainActivity_getScreenWidth(JNIEnv* env, jobject self) {
    // FIXME: read from actual window
    return 1600;
}

extern "C" jint JNICALL MainActivity_getScreenHeight(JNIEnv* env, jobject self) {
    return 1200;
}

extern "C" jint JNICALL MainActivity_getDisplayWidth(JNIEnv* env, jobject self) {
    return MainActivity_getScreenWidth(env, self);
}

extern "C" jint JNICALL MainActivity_getDisplayHeight(JNIEnv* env, jobject self) {
    return MainActivity_getScreenHeight(env, self);
}

extern "C" void JNICALL MainActivity_tick(JNIEnv* env, jobject self) {}

extern "C" jboolean JNICALL MainActivity_isNetworkEnabled(JNIEnv* env, jobject self, jboolean wifi) {
    return JNI_TRUE;
}

extern "C" jboolean JNICALL MainActivity_isChromebook(JNIEnv* env, jobject self) {
    return JNI_TRUE;
}

extern "C" jstring JNICALL MainActivity_getLocale(JNIEnv* env, jobject self) {
    return env->NewStringUTF("en");
}

extern "C" jstring JNICALL MainActivity_getDeviceModel(JNIEnv* env, jobject self) {
    return env->NewStringUTF("Linux");
}

extern "C" jobject JNICALL MainActivity_getFilesDir(JNIEnv* env, jobject self) {
    auto* f = new FileObject();
    strncpy(f->path, g_storage_dir, sizeof(f->path) - 1);
    return (jobject)f;
}

extern "C" jobject JNICALL MainActivity_getCacheDir(JNIEnv* env, jobject self) {
    return MainActivity_getFilesDir(env, self);
}

extern "C" jstring JNICALL MainActivity_getExternalStoragePath(JNIEnv* env, jobject self) {
    return env->NewStringUTF(g_storage_dir);
}

extern "C" jstring JNICALL MainActivity_getInternalStoragePath(JNIEnv* env, jobject self) {
    return MainActivity_getExternalStoragePath(env, self);
}

extern "C" jstring JNICALL MainActivity_getLegacyExternalStoragePath(JNIEnv* env, jobject self, jstring gameFolder) {
    return env->NewStringUTF("");
}

extern "C" jboolean JNICALL MainActivity_hasWriteExternalStoragePermission(JNIEnv* env, jobject self) {
    return JNI_TRUE;
}

extern "C" jboolean JNICALL MainActivity_hasReadMediaImagesPermission(JNIEnv* env, jobject self) {
    return JNI_TRUE;
}

extern "C" jobject JNICALL MainActivity_getHardwareInfo(JNIEnv* env, jobject self) {
    // Create and return HardwareInfo object
    return (jobject)new char[1]; // dummy - HardwareInfo has no fields
}

extern "C" jfloat JNICALL MainActivity_getPixelsPerMillimeter(JNIEnv* env, jobject self) {
    return (96.0f / 25.4f) * 2.0f;
}

extern "C" jint JNICALL MainActivity_getPlatformDpi(JNIEnv* env, jobject self) {
    return 96 * 2;
}

extern "C" jobject JNICALL MainActivity_createUUID(JNIEnv* env, jobject self) {
    // Call UUID.randomUUID() through JNI
    jclass uuidClass = env->FindClass("java/util/UUID");
    if (!uuidClass) return nullptr;
    jmethodID randomUUID = env->GetStaticMethodID(uuidClass, "randomUUID", "()Ljava/util/UUID;");
    if (!randomUUID) return nullptr;
    return env->CallStaticObjectMethod(uuidClass, randomUUID);
}

extern "C" jobject JNICALL MainActivity_getIPAddresses(JNIEnv* env, jobject self) {
    // Return empty array
    jclass stringClass = env->FindClass("java/lang/String");
    if (!stringClass) return nullptr;
    return env->NewObjectArray(0, stringClass, nullptr);
}

extern "C" void JNICALL MainActivity_runNativeCallbackOnUiThread(JNIEnv* env, jobject self, jlong h) {
    // Call native method via reflection
    jclass cls = env->GetObjectClass(self);
    jmethodID mid = env->GetMethodID(cls, "nativeRunNativeCallbackOnUiThread", "(J)V");
    if (mid) env->CallVoidMethod(self, mid, h);
}

extern "C" void JNICALL MainActivity_showKeyboard(JNIEnv* env, jobject self, jstring text, jint maxLen, jboolean ignored1, jboolean ignored2, jboolean multiline) {
    fprintf(stderr, "[MainActivity] showKeyboard\n");
}

extern "C" void JNICALL MainActivity_hideKeyboard(JNIEnv* env, jobject self) {
    fprintf(stderr, "[MainActivity] hideKeyboard\n");
}

extern "C" jboolean JNICALL MainActivity_hasHardwareKeyboard(JNIEnv* env, jobject self) {
    return JNI_TRUE;
}

extern "C" jint JNICALL MainActivity_getCursorPosition(JNIEnv* env, jobject self) {
    return 0;
}

extern "C" jstring JNICALL MainActivity_getTextBoxBackend(JNIEnv* env, jobject self) {
    return env->NewStringUTF("");
}

extern "C" void JNICALL MainActivity_setCaretPosition(JNIEnv* env, jobject self, jint pos) {}

extern "C" jlong JNICALL MainActivity_calculateAvailableDiskFreeSpace(JNIEnv* env, jobject self, jstring str) {
    return 1024LL * 1024LL * 1024LL * 1024LL;
}

extern "C" jlong JNICALL MainActivity_getUsableSpace(JNIEnv* env, jobject self, jstring str) {
    return 1024LL * 1024LL * 1024LL * 1024LL;
}

extern "C" jint JNICALL MainActivity_getCaretPosition(JNIEnv* env, jobject self) {
    return 0;
}

extern "C" jlong JNICALL MainActivity_getUsedMemory(JNIEnv* env, jobject self) {
    return 0;
}

extern "C" jlong JNICALL MainActivity_getFreeMemory(JNIEnv* env, jobject self) {
    FILE* fp = fopen("/proc/meminfo", "r");
    if (!fp) return 0;
    long total = 0, free = 0;
    char line[256];
    while (fgets(line, sizeof(line), fp)) {
        if (sscanf(line, "MemTotal: %ld", &total) == 1) continue;
        if (sscanf(line, "MemAvailable: %ld", &free) == 1) continue;
    }
    fclose(fp);
    return free * 1024;
}

extern "C" jlong JNICALL MainActivity_getTotalMemory(JNIEnv* env, jobject self) {
    FILE* fp = fopen("/proc/meminfo", "r");
    if (!fp) return 0;
    long total = 0;
    char line[256];
    while (fgets(line, sizeof(line), fp)) {
        if (sscanf(line, "MemTotal: %ld", &total) == 1) break;
    }
    fclose(fp);
    return total * 1024;
}

extern "C" jlong JNICALL MainActivity_getMemoryLimit(JNIEnv* env, jobject self) {
    return MainActivity_getTotalMemory(env, self) * 2;
}

extern "C" jlong JNICALL MainActivity_getAvailableMemory(JNIEnv* env, jobject self) {
    return MainActivity_getFreeMemory(env, self);
}

extern "C" void JNICALL MainActivity_pickImage(JNIEnv* env, jobject self, jlong callback) {
    // No-op
}

extern "C" void JNICALL MainActivity_setClipboard(JNIEnv* env, jobject self, jstring text) {}

extern "C" void JNICALL MainActivity_initializeXboxLive(JNIEnv* env, jobject self, jlong xalinit, jlong xblinit) {
    fprintf(stderr, "[MainActivity] initializeXboxLive\n");
}

extern "C" jlong JNICALL MainActivity_initializeXboxLive2(JNIEnv* env, jobject self, jlong xalinit, jlong xblinit) {
    return 0;
}

extern "C" jlong JNICALL MainActivity_initializeLibHttpClient(JNIEnv* env, jobject self, jlong init) {
    return 0;
}

extern "C" void JNICALL MainActivity_startPlayIntegrityCheck(JNIEnv* env, jobject self) {}

extern "C" void JNICALL MainActivity_openFile(JNIEnv* env, jobject self) {}

extern "C" void JNICALL MainActivity_saveFile(JNIEnv* env, jobject self, jstring str) {}

extern "C" void JNICALL MainActivity_setLastChar(JNIEnv* env, jobject self, jint sym) {}

extern "C" jlong JNICALL MainActivity_getAllocatableBytes(JNIEnv* env, jobject self, jstring path) {
    return MainActivity_getFreeMemory(env, self);
}

extern "C" jboolean JNICALL MainActivity_supportsSizeQuery(JNIEnv* env, jobject self, jstring path) {
    return JNI_FALSE;
}

extern "C" void JNICALL MainActivity_requestIntegrityToken(JNIEnv* env, jobject self, jstring str) {}

extern "C" void JNICALL MainActivity_launchUri(JNIEnv* env, jobject self, jstring uri) {}

extern "C" void JNICALL MainActivity_share(JNIEnv* env, jobject self, jstring a, jstring b, jstring c) {}

extern "C" void JNICALL MainActivity_shareFile(JNIEnv* env, jobject self, jstring a, jstring b, jstring c) {}

extern "C" jobject JNICALL MainActivity_getBroadcastAddresses(JNIEnv* env, jobject self) {
    jclass stringClass = env->FindClass("java/lang/String");
    if (!stringClass) return nullptr;
    return env->NewObjectArray(0, stringClass, nullptr);
}

extern "C" void JNICALL MainActivity_updateTextboxText(JNIEnv* env, jobject self, jstring newText) {}

extern "C" void JNICALL MainActivity_setTextBoxBackend(JNIEnv* env, jobject self, jstring newText) {}

extern "C" jint JNICALL MainActivity_getKeyFromKeyCode(JNIEnv* env, jobject self, jint keyCode, jint metaState, jint deviceId) {
    return keyCode; // pass through
}

extern "C" void JNICALL MainActivity_lockCursor(JNIEnv* env, jobject self) {}

extern "C" void JNICALL MainActivity_unlockCursor(JNIEnv* env, jobject self) {}

extern "C" jobject JNICALL MainActivity_getImageData(JNIEnv* env, jobject self, jstring filename) {
    return nullptr;
}

extern "C" jlong JNICALL MainActivity_getFileDataBytes(JNIEnv* env, jobject self, jstring path) {
    // Returns ByteArray - for now return null to avoid complexity
    return 0;
}

static void register_main_activity_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"getAndroidVersion",            (char*)"()I",                              (void*)&MainActivity_getAndroidVersion},
        {(char*)"getScreenWidth",               (char*)"()I",                              (void*)&MainActivity_getScreenWidth},
        {(char*)"getScreenHeight",              (char*)"()I",                              (void*)&MainActivity_getScreenHeight},
        {(char*)"getDisplayWidth",              (char*)"()I",                              (void*)&MainActivity_getDisplayWidth},
        {(char*)"getDisplayHeight",             (char*)"()I",                              (void*)&MainActivity_getDisplayHeight},
        {(char*)"tick",                          (char*)"()V",                              (void*)&MainActivity_tick},
        {(char*)"isNetworkEnabled",             (char*)"(Z)Z",                             (void*)&MainActivity_isNetworkEnabled},
        {(char*)"isChromebook",                 (char*)"()Z",                              (void*)&MainActivity_isChromebook},
        {(char*)"getLocale",                    (char*)"()Ljava/lang/String;",            (void*)&MainActivity_getLocale},
        {(char*)"getDeviceModel",               (char*)"()Ljava/lang/String;",            (void*)&MainActivity_getDeviceModel},
        {(char*)"getFilesDir",                  (char*)"()Ljava/io/File;",                (void*)&MainActivity_getFilesDir},
        {(char*)"getCacheDir",                  (char*)"()Ljava/io/File;",                (void*)&MainActivity_getCacheDir},
        {(char*)"getExternalStoragePath",       (char*)"()Ljava/lang/String;",            (void*)&MainActivity_getExternalStoragePath},
        {(char*)"getInternalStoragePath",        (char*)"()Ljava/lang/String;",            (void*)&MainActivity_getInternalStoragePath},
        {(char*)"getLegacyExternalStoragePath",  (char*)"(Ljava/lang/String;)Ljava/lang/String;", (void*)&MainActivity_getLegacyExternalStoragePath},
        {(char*)"hasWriteExternalStoragePermission", (char*)"()Z",                         (void*)&MainActivity_hasWriteExternalStoragePermission},
        {(char*)"hasReadMediaImagesPermission",     (char*)"()Z",                          (void*)&MainActivity_hasReadMediaImagesPermission},
        {(char*)"getHardwareInfo",              (char*)"()Lcom/mojang/minecraftpe/HardwareInformation;", (void*)&MainActivity_getHardwareInfo},
        {(char*)"getPixelsPerMillimeter",       (char*)"()F",                             (void*)&MainActivity_getPixelsPerMillimeter},
        {(char*)"getPlatformDpi",               (char*)"()I",                              (void*)&MainActivity_getPlatformDpi},
        {(char*)"createUUID",                   (char*)"()Ljava/util/UUID;",              (void*)&MainActivity_createUUID},
        {(char*)"getIPAddresses",               (char*)"()[Ljava/lang/String;",           (void*)&MainActivity_getIPAddresses},
        {(char*)"runNativeCallbackOnUiThread",   (char*)"(J)V",                            (void*)&MainActivity_runNativeCallbackOnUiThread},
        {(char*)"showKeyboard",                 (char*)"(Ljava/lang/String;IZZZ)V",       (void*)&MainActivity_showKeyboard},
        {(char*)"hideKeyboard",                 (char*)"()V",                              (void*)&MainActivity_hideKeyboard},
        {(char*)"hasHardwareKeyboard",          (char*)"()Z",                              (void*)&MainActivity_hasHardwareKeyboard},
        {(char*)"getCursorPosition",            (char*)"()I",                              (void*)&MainActivity_getCursorPosition},
        {(char*)"getTextBoxBackend",            (char*)"()Ljava/lang/String;",            (void*)&MainActivity_getTextBoxBackend},
        {(char*)"setCaretPosition",             (char*)"(I)V",                             (void*)&MainActivity_setCaretPosition},
        {(char*)"calculateAvailableDiskFreeSpace", (char*)"(Ljava/lang/String;)J",        (void*)&MainActivity_calculateAvailableDiskFreeSpace},
        {(char*)"getUsableSpace",               (char*)"(Ljava/lang/String;)J",            (void*)&MainActivity_getUsableSpace},
        {(char*)"getCaretPosition",             (char*)"()I",                              (void*)&MainActivity_getCaretPosition},
        {(char*)"getUsedMemory",                (char*)"()J",                              (void*)&MainActivity_getUsedMemory},
        {(char*)"getFreeMemory",                (char*)"()J",                              (void*)&MainActivity_getFreeMemory},
        {(char*)"getTotalMemory",               (char*)"()J",                              (void*)&MainActivity_getTotalMemory},
        {(char*)"getMemoryLimit",               (char*)"()J",                              (void*)&MainActivity_getMemoryLimit},
        {(char*)"getAvailableMemory",           (char*)"()J",                              (void*)&MainActivity_getAvailableMemory},
        {(char*)"pickImage",                    (char*)"(J)V",                             (void*)&MainActivity_pickImage},
        {(char*)"setClipboard",                 (char*)"(Ljava/lang/String;)V",            (void*)&MainActivity_setClipboard},
        {(char*)"initializeXboxLive",           (char*)"(JJ)V",                            (void*)&MainActivity_initializeXboxLive},
        {(char*)"initializeXboxLive2",          (char*)"(JJ)J",                            (void*)&MainActivity_initializeXboxLive2},
        {(char*)"initializeLibHttpClient",      (char*)"(J)J",                             (void*)&MainActivity_initializeLibHttpClient},
        {(char*)"startPlayIntegrityCheck",      (char*)"()V",                              (void*)&MainActivity_startPlayIntegrityCheck},
        {(char*)"openFile",                     (char*)"()V",                              (void*)&MainActivity_openFile},
        {(char*)"saveFile",                     (char*)"(Ljava/lang/String;)V",            (void*)&MainActivity_saveFile},
        {(char*)"setLastChar",                  (char*)"(I)V",                             (void*)&MainActivity_setLastChar},
        {(char*)"getAllocatableBytes",          (char*)"(Ljava/lang/String;)J",            (void*)&MainActivity_getAllocatableBytes},
        {(char*)"supportsSizeQuery",            (char*)"(Ljava/lang/String;)Z",            (void*)&MainActivity_supportsSizeQuery},
        {(char*)"requestIntegrityToken",        (char*)"(Ljava/lang/String;)V",            (void*)&MainActivity_requestIntegrityToken},
        {(char*)"launchUri",                    (char*)"(Ljava/lang/String;)V",            (void*)&MainActivity_launchUri},
        {(char*)"share",                        (char*)"(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V", (void*)&MainActivity_share},
        {(char*)"shareFile",                    (char*)"(Ljava/lang/String;Ljava/lang/String;Ljava/lang/String;)V", (void*)&MainActivity_shareFile},
        {(char*)"getBroadcastAddresses",        (char*)"()[Ljava/lang/String;",           (void*)&MainActivity_getBroadcastAddresses},
        {(char*)"updateTextboxText",            (char*)"(Ljava/lang/String;)V",            (void*)&MainActivity_updateTextboxText},
        {(char*)"setTextBoxBackend",            (char*)"(Ljava/lang/String;)V",            (void*)&MainActivity_setTextBoxBackend},
        {(char*)"getKeyFromKeyCode",            (char*)"(III)I",                           (void*)&MainActivity_getKeyFromKeyCode},
        {(char*)"lockCursor",                   (char*)"()V",                              (void*)&MainActivity_lockCursor},
        {(char*)"unlockCursor",                 (char*)"()V",                              (void*)&MainActivity_unlockCursor},
        {(char*)"getImageData",                 (char*)"(Ljava/lang/String;)[I",           (void*)&MainActivity_getImageData},
        {(char*)"getFileDataBytes",             (char*)"(Ljava/lang/String;)[B",           (void*)&MainActivity_getFileDataBytes},
    };
    register_class(env, "com/mojang/minecraftpe/MainActivity", methods,
        sizeof(methods) / sizeof(methods[0]));
}

// ================================================================
// com/mojang/minecraftpe/input/JellyBeanDeviceManager
// ================================================================

extern "C" void JNICALL JellyBeanDeviceManager_onInputDeviceAddedNative(JNIEnv* env, jobject self, jint devId) {}
extern "C" void JNICALL JellyBeanDeviceManager_onInputDeviceRemovedNative(JNIEnv* env, jobject self, jint devId) {}

static void register_jelly_bean_device_manager_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"onInputDeviceAddedNative",   (char*)"(I)V", (void*)&JellyBeanDeviceManager_onInputDeviceAddedNative},
        {(char*)"onInputDeviceRemovedNative", (char*)"(I)V", (void*)&JellyBeanDeviceManager_onInputDeviceRemovedNative},
    };
    register_class(env, "com/mojang/minecraftpe/input/JellyBeanDeviceManager", methods, 2);
}

// ================================================================
// com/mojang/minecraftpe/PlayIntegrity
// ================================================================

extern "C" void JNICALL PlayIntegrity_nativePlayIntegrityComplete(JNIEnv* env, jobject self) {}

static void register_play_integrity_class(JNIEnv* env) {
    JNINativeMethod methods[] = {
        {(char*)"nativePlayIntegrityComplete", (char*)"()V", (void*)&PlayIntegrity_nativePlayIntegrityComplete},
    };
    register_class(env, "com/mojang/minecraftpe/PlayIntegrity", methods, 1);
}

// ================================================================
// Main registration entry point — called from Rust jni_support.rs
// ================================================================

extern "C" void register_all_jnivm_classes(void* env_ptr) {
    JNIEnv* env = (JNIEnv*)env_ptr;
    if (!env) {
        fprintf(stderr, "[jnivm_wrapper] register_all_jnivm_classes: null env!\n");
        return;
    }

    fprintf(stderr, "[jnivm_wrapper] Registering all Java classes with libjnivm-sys...\n");

    register_file_class(env);
    register_build_version_class(env);
    register_package_info_class(env);
    register_package_manager_class(env);
    register_context_class(env);
    register_hardware_info_class(env);
    register_network_monitor_class(env);
    register_main_activity_class(env);
    register_jelly_bean_device_manager_class(env);
    register_play_integrity_class(env);

    fprintf(stderr, "[jnivm_wrapper] All Java classes registered with libjnivm-sys\n");
}

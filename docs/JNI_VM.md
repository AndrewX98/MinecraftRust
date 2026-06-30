# JNI VM Architecture

The launcher uses a **dual JNI VM** setup during transition. The Rust libjnivm-sys VM is the **active dispatch VM** for all game JNI operations, while the C++ FakeJni VM is kept for compatibility with the game's cached references and FakeLooper callback dispatch.

## The Two VMs

### 1. libjnivm-sys VM (Rust, ACTIVE for ALL JNI)

- **Created by**: `jni_support::jni_support_new()` in `jni_support.rs:198`
- **Type**: Pure Rust JNI VM (`libjnivm-sys` crate, 25 source modules)
- **Registration API**: `class::register(env)` for Rust modules, `register_all_jnivm_classes(env)` for C++ wrappers
- **Native registration**: `jni_support_register_natives()` in `jni_support.rs:236`
- **Startup orchestration**: `jni_support_start_game()` in `jni_support.rs:493` — called from `main.rs:110`
- **Env switch**: `(*ga).env = get_env()` in `jni_support.rs:402` — game now dispatches all JNI calls through libjnivm-sys
- **Status**: **ACTIVE** — creates the VM, registers all Java classes + native methods, and serves as the game's `env` pointer for all JNI dispatch

### 2. FakeJni / Baron VM (C++, LEGACY — kept for compatibility)

- **Created by**: `JniSupport::JniSupport()` in `jni_support.cpp`, accessed via `jni_support_get_jvm()` from Rust
- **Type**: `FakeJni::Jvm` (C++ class, part of libjnivm C++ library, compiled locally by `build.rs`)
- **Game receives**: During `jni_support_start_game_with_baron()`, `ga->vm` is set to Baron VM (for `vm` operations) but `ga->env` now points to libjnivm-sys
- **Class registration**: C++ classes registered via `vm.registerClass<T>()` in `registerJniClasses()`
- **Native registration**: `registerMinecraftNatives()` in `jni_support.cpp` — still called during startup before Rust VM setup
- **Status**: **LEGACY** — only needed for `FakeLooper::onGameActivityClose` callback and any C++ code that accesses the Baron VM directly. All game JNI dispatch now goes through libjnivm-sys.

## Class Registration

### libjnivm-sys VM (Rust) — via `register_all_classes()` in `jni_support.rs:130`

**Rust modules** (8 modules, in `jni_support.rs` + `crates/client/src/jni/`):

| Module | Java Class | Methods |
|--------|------------|---------|
| `uuid` | `java/util/UUID` | randomUUID, makeRandomUUID, toString |
| `locale` | `java/util/Locale` | getDefault, toString |
| `certificate` | `java/security/cert/CertificateFactory` | getInstance, generateCertificate |
| `certificate` | `javax/net/ssl/TrustManagerFactory` | getInstance, getTrustManagers |
| `certificate` | `org/apache/http/conn/ssl/StrictHostnameVerifier` | verify |
| `ecdsa_impl` | `com/microsoft/xal/crypto/Ecdsa` | \<init\>, generateKey, sign, getPublicKey, getUniqueId, restoreKeyAndId |
| `ecdsa_impl` | `com/microsoft/xal/crypto/EccPubKey` | getBase64UrlX, getBase64UrlY |
| `store` | `com/mojang/minecraftpe/store/Store` | receivedLicenseResponse, hasVerifiedLicense, getStoreId, getProductSkuPrefix, getRealmsSkuPrefix, getExtraLicenseData, queryProducts, purchase, acknowledgePurchase, queryPurchases, destructor |
| `store` | `com/mojang/minecraftpe/store/NativeStoreListener` | onStoreInitialized, onPurchaseFailed, onQueryProductsSuccess, onQueryPurchasesSuccess |
| `store` | `com/mojang/minecraftpe/store/ExtraLicenseResponseData` | getValidationTime, getRetryUntilTime, getRetryAttempts |
| `store` | `com/mojang/minecraftpe/NotificationListenerService` | getDeviceRegistrationToken |
| `store` | `com/mojang/minecraftpe/store/StoreFactory` | createGooglePlayStore, createAmazonAppStore |
| `audio` | `org/fmod/AudioDevice` | init, write, write2, close |
| `http_client` | `com/xbox/httpclient/HttpClientRequest` | \<init\>, destroy, isNetworkAvailable, createClientRequest, setHttpUrl, setHttpMethodAndBody, setHttpHeader, doRequestAsync |
| `http_client` | `com/xbox/httpclient/HttpClientResponse` | getNumHeaders, getHeaderNameAtIndex, getHeaderValueAtIndex, getResponseBodyBytes, getResponseCode |
| `websocket` | `com/xbox/httpclient/HttpClientWebSocket` | \<init\>, destroy, connect, addHeader, sendMessage, sendBinaryMessage, disconnect |

Plus 5 "ensured" classes (FindClass only): InputStream, ByteArrayInputStream, Certificate, TrustManager, X509TrustManager

**Rust `main_activity.rs`** — all 57 MainActivity methods (replaced C++ `main_activity.cpp`):

| Java Class | Methods |
|------------|---------|
| `com/mojang/minecraftpe/MainActivity` | getScreenWidth, getScreenHeight, getLocale, getLanguage, getCountryCode, getAppVersion, getAppVersionCode, getPackageName, getPackageCodePath, getPackageIcon, getPackageIconPath, getAndroidPath, getExternalStoragePath, getSecondaryExternalStoragePath, getExternalStorageState, getCachesDir, getFilesDir, getDataDir, getExternalFilesDir, getExternalCacheDirs, getObbDirs, getDatabasePath, getPreferredSkinPackId, getCurrentTheme, getRealmTheme, isPackageInstalled, areHomescreenItemsAvailable, canRequestPackageInstalls, getLauncherInfo, getUsedMemory, startActivity, startActivityForResult, showKeyboard, hideKeyboard, dismissKeyboard, getBatteryLevel, setBatteryLevel, getBatteryStatus, setBatteryStatus, getMaxNumberOfPlayersForWorld, getMaxNumberOfPlayersForWorldList, showToast, setClipboardText, getClipboardText, getDayOfWeek, getElapsedRealtime, getAvailableMemory, getConnectedWifiSsid, getActiveTextureMemorySize, getPrevActiveTextureMemorySize, getRot, setGameControllerConnected, setGamepadMapping, onGameActivityClose, createUUID, lockCursor, unlockCursor, pickImage, openFile, saveFile, launchUri, share, shareFile, getImageData, runNativeCallbackOnUiThread, requestIntegrityToken |

**Rust `jnivm_class_wrappers.rs`** — 21 methods across 9 classes (coexists with `jnivm_class_wrappers.cpp`):

| Java Class | Methods |
|------------|---------|
| `java/io/File` | getPath, getAbsolutePath, exists, length, isDirectory |
| `android/os/Build$VERSION` | getSdkInt, getRelease |
| `android/content/pm/PackageInfo` | getVersionName |
| `android/content/pm/PackageManager` | getPackageInfo |
| `android/content/Context` | getFilesDir, getCacheDir, getClassLoader, getApplicationContext, getPackageName, getPackageManager |
| `com/mojang/minecraftpe/HardwareInformation` | getAndroidVersion, getInstallerPackageName |
| `com/mojang/minecraftpe/NetworkMonitor` | nativeUpdateNetworkStatus |
| `com/mojang/minecraftpe/input/JellyBeanDeviceManager` | onInputDeviceAddedNative, onInputDeviceRemovedNative |
| `com/mojang/minecraftpe/PlayIntegrity` | nativePlayIntegrityComplete |

**C++ wrapper** (still compiled, via `jnivm_class_wrappers.cpp`):
Same 10 classes registered redundantly — kept because `jni_support.cpp` calls `registerClass<File>()` etc. via the FakeJni VM during `registerJniClasses()`. The Rust versions are the ones actually serving JNI calls.

### FakeJni VM (C++) — via `registerJniClasses()` in `jni_support.cpp:186`

All these classes are still registered with `vm.registerClass<T>()` for FakeJni compatibility (FakeLooper callback dispatch):

```
File, ClassLoader, Locale,
BuildVersion, PackageInfo, PackageManager, Context,
ContextWrapper, HardwareInfo, Activity, NativeActivity,
NetworkMonitor, MainActivity, AccountManager, Account,
StoreListener, NativeStoreListener, Store, StoreFactory,
ExtraLicenseResponseData,
XboxInterop, XboxLocalStorage,
HttpClientRequest, HttpClientResponse, HttpClientWebSocket,
PackageSource, PackageSourceListener, NativePackageSourceListener,
PackageSourceFactory,
ShaHasher, SecureRandom,
WebView, BrowserLaunchActivity,
JBase64, Arrays, Signature, PublicKey,
Product, Purchase, NotificationListenerService,
PlayIntegrity,
FMOD, AssetManager,
EventTracerHelperMultiplayer,
AudioDevice, AndroidJniHelperMultiplayer
```

## Native Method Registration (Both VMs)

Both VMs register the same native methods — symbols resolved from the loaded `libminecraftpe.so` library:

### libjnivm-sys (Rust, `jni_support_register_natives()`)
```rust
// 13 classes registered via jnivm_find_class + jnivm_register_natives
// Uses jni_resolve_symbol → mc_dlsym for Java_* symbols
```

### FakeJni (C++, `registerMinecraftNatives()`)
```cpp
registerNatives(MainActivity::getDescriptor(), {{"nativeRegisterThis", "()V"}, ...}, symResolver);
// Same 13 classes registered redundantly — kept for compatibility
```

## Key Insight: Env Switch Architecture

1. **libjnivm-sys (Rust)** is now the **primary JNI dispatch VM**. The critical env switch (`(*ga).env = get_env()`) means the game sends all `CallXxxMethod`, `CallStaticXxxMethod`, `FindClass`, `GetMethodID`, etc. through the Rust 250-function vtable.

2. **FakeJni/Baron (C++)** is **legacy** — the game's `vm` pointer still points to Baron (for operations like `AttachCurrentThread`), and `FakeLooper::onGameActivityClose` uses FakeJni class resolution. But the main JNI dispatch path is fully Rust.

3. **Registration redundancy**: Both VMs still register the same classes and natives. The C++ `registerJniClasses()` runs first (step 11 in startup), then Rust `register_all_classes()` runs (step 14). This redundancy is a transitional requirement — `jni_support.cpp` must still compile and link.

4. **`jnivm_globals.rs`** provides `#[no_mangle]` extern "C" getter/setter functions that the C++ bridge code calls to access global state (window handle, storage dir, text input handler, asset manager, stbi function pointers). Previously these lived in `jnivm_class_wrappers.cpp`.

5. **BARON_ENV** global (`jni_support.rs:107`): A `OnceLock<Mutex<Option<SendPtr<c_void>>>>` that stores the Baron `JNIEnv` pointer. Set via `set_baron_env()` (line 109) during `jni_support_start_game_with_baron()` — the Baron `LocalFrame` is created **before** library attachment (line 403) to ensure XSAPI's `JNI_OnLoad` background threads have safe env access. Retrieved via `get_baron_env()` (line 115) by any code needing the Baron env for FakeJni callbacks.

## Two-VM Coexistence (during transition)

| Feature | libjnivm-sys (Rust) | FakeJni/Baron (C++) |
|---------|-------------------|---------------------|
| JNI dispatch (`env->Call*`) | ✅ ACTIVE | ❌ Not used (game env points to Rust) |
| Class registration | ✅ ACTIVE | 🟡 Redundant (kept for linker) |
| Native method registration | ✅ ACTIVE | 🟡 Redundant (kept for linker) |
| `vm` operations (AttachCurrentThread) | 🟡 Not available | ✅ Active (game `ga->vm` = Baron) |
| FakeLooper callback dispatch | ❌ Not used | ✅ Active (`onGameActivityClose`) |
| Game `FindClass` | ✅ Active (Rust classes) | 🟡 Redundant (still registered) |

## Porting Pattern

For each C++ JNI class:

1. Keep FakeJni registrations in `jni_support.cpp` **unchanged** (FakeJni still linked for compatibility)
2. Add `.cpp` to `excluded_jni` in `build.rs`
3. If the C++ class is also registered with Baron, create a `_stub.cpp` with minimal method bodies
4. Write the real implementation in Rust (register with libjnivm-sys)
5. Register the Rust implementation via `RegisterNatives` on the libjnivm-sys VM

With the env switch complete, step 4+5 are the only ones that matter for game behavior — but step 1 is still required to keep linking working.

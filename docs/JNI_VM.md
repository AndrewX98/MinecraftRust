# JNI VM Architecture

The launcher uses a **hybrid JNI VM approach**: a pure Rust libjnivm-sys VM for class registration and native method registration, with the C++ Baron JVM (FakeJni) handling game activity JNI dispatch via `jni_support_start_game_with_baron()`.

## The Two VMs

### 1. libjnivm-sys VM (Rust, ACTIVE for class/native registration)

- **Created by**: `jni_support::jni_support_new()` in `jni_support.rs:198`
- **Type**: Pure Rust JNI VM (`libjnivm-sys` crate, 25 source modules)
- **Registration API**: `class::register(env)` for Rust modules, `register_all_jnivm_classes(env)` for C++ wrappers
- **Native registration**: `jni_support_register_natives()` in `jni_support.rs:236`
- **Startup orchestration**: `jni_support_start_game()` in `jni_support.rs:493` — called from `main.rs:110`
- **Status**: **ACTIVE** — creates the VM, registers all Java classes + native methods, then delegates game activity dispatch to Baron (FakeJni)

### 2. FakeJni / Baron VM (C++, ACTIVE for game dispatch)

- **Created by**: `JniSupport::JniSupport()` in `jni_support.cpp`, accessed via `jni_support_get_jvm()` from Rust
- **Type**: `FakeJni::Jvm` (C++ class, part of libjnivm C++ library, compiled locally by `build.rs`)
- **Game receives**: Set during `jni_support_start_game_with_baron()` — `ga->vm = baron_vm; ga->env = baron_env;`
- **Class registration**: C++ classes registered via `vm.registerClass<T>()` in `registerJniClasses()` (e.g. MainActivity, Store, etc.)
- **Native registration**: `registerMinecraftNatives()` for game native methods (called before Rust VM setup)
- **Status**: **ACTIVE FOR GAME DISPATCH** — `GameActivity_onCreate` caches Baron's vm/env pointers, so all game `env->CallXxxMethod()` / `env->FindClass()` calls go through Baron. The Rust libjnivm-sys VM handles class definitions and native method lookup.

## Class Registration

### FakeJni VM (C++) — via `registerJniClasses()` in `jni_support.cpp:186`

All these classes are registered with `vm.registerClass<T>()`:

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

### libjnivm-sys VM (Rust) — via `register_all_classes()` in `jni_support.rs:130`

**Rust modules** (4 modules, in `jni_support.rs`):

| Module | Java Class | Methods |
|--------|------------|---------|
| `uuid` | `java/util/UUID` | randomUUID, makeRandomUUID, toString |
| `locale` | `java/util/Locale` | getDefault, toString |
| `certificate` | `java/security/cert/CertificateFactory` | getInstance, generateCertificate |
| `certificate` | `javax/net/ssl/TrustManagerFactory` | getInstance, getTrustManagers |
| `certificate` | `org/apache/http/conn/ssl/StrictHostnameVerifier` | verify |
| `ecdsa_impl` | `com/microsoft/xal/crypto/Ecdsa` | \<init\>, generateKey, sign, getPublicKey, getUniqueId, restoreKeyAndId |
| `ecdsa_impl` | `com/microsoft/xal/crypto/EccPubKey` | getBase64UrlX, getBase64UrlY |

Plus 5 "ensured" classes (FindClass only): InputStream, ByteArrayInputStream, Certificate, TrustManager, X509TrustManager

**C++ wrapper** (via `register_all_jnivm_classes()` in `jnivm_class_wrappers.cpp:620`):

| Java Class | Methods |
|------------|---------|
| `java/io/File` | getPath, getAbsolutePath, exists, length, isDirectory |
| `android/os/Build$VERSION` | getSdkInt, getRelease |
| `android/content/pm/PackageInfo` | getVersionName |
| `android/content/pm/PackageManager` | getPackageInfo |
| `android/content/Context` | getFilesDir, getCacheDir, getClassLoader, getApplicationContext, getPackageName, getPackageManager |
| `com/mojang/minecraftpe/HardwareInformation` | getAndroidVersion, getInstallerPackageName |
| `com/mojang/minecraftpe/NetworkMonitor` | nativeUpdateNetworkStatus |
| `com/mojang/minecraftpe/MainActivity` | ~60 methods (getScreenWidth, getLocale, getUsedMemory, showKeyboard, createUUID, etc.) |
| `com/mojang/minecraftpe/input/JellyBeanDeviceManager` | onInputDeviceAddedNative, onInputDeviceRemovedNative |
| `com/mojang/minecraftpe/PlayIntegrity` | nativePlayIntegrityComplete |

## Native Method Registration (Both VMs)

Both VMs register the same native methods — symbols resolved from the loaded `libminecraftpe.so` library:

### FakeJni (C++, `registerMinecraftNatives()`)
```cpp
registerNatives(MainActivity::getDescriptor(), {{"nativeRegisterThis", "()V"}, ...}, symResolver);
registerNatives(NetworkMonitor::getDescriptor(), ...);
registerNatives(NativeStoreListener::getDescriptor(), ...);
registerNatives(JellyBeanDeviceManager::getDescriptor(), ...);
registerNatives(HttpClientRequest::getDescriptor(), ...);
registerNatives(HttpClientWebSocket::getDescriptor(), ...);
registerNatives(WebView::getDescriptor(), ...);
registerNatives(BrowserLaunchActivity::getDescriptor(), ...);
registerNatives(NativeInputStream::getDescriptor(), ...);
registerNatives(NativeOutputStream::getDescriptor(), ...);
registerNatives(NetworkObserver::getDescriptor(), ...);
registerNatives(PlayIntegrity::getDescriptor(), ...);
```

### libjnivm-sys (Rust, `jni_support_register_natives()`)
```rust
// Same 13 classes, same method names and signatures
// Uses jnivm_find_class + jnivm_register_natives + symbol resolver
```

## Key Insight: Hybrid JNI Architecture

1. **libjnivm-sys (Rust)** is the **class/native registration VM**. It creates all Java class definitions (`FindClass` + `RegisterNatives`) and resolves game native method symbols. The Rust startup path creates it first, then delegates to Baron for the game activity lifecycle.

2. **FakeJni/Baron (C++)** is the **game's JNI dispatch VM**. When the game calls `env->FindClass()` or `env->CallVoidMethod()` during gameplay, it uses the Baron `JNIEnv*` cached on `gameActivity.env` during `GameActivity_onCreate`. Baron holds the actual class instances and method dispatch tables.

3. **The Rust `jni_support_start_game()` function** (jni_support.rs:493) is called from `main.rs:110`. It:
   - Creates the libjnivm-sys VM and registers all classes/natives
   - Calls `jni_support_start_game_with_baron()` (Rust, jni_support.rs:359) which:
     - Gets the Baron JVM from C++ JniSupport
     - Creates a Baron LocalFrame 
     - Sets `ga->vm` and `ga->env` to Baron pointers
     - Calls `GameActivity_onCreate` (game caches Baron vm/env)
     - Dispatches `onStart` / `onNativeWindowCreated`

## Current Hybrid State

The hybrid architecture means:

1. **FakeJni VM still exists** — created by C++ `JniSupport::JniSupport()`, accessed from Rust via `jni_support_get_jvm()`. Baron handles game activity dispatch in `jni_support_start_game_with_baron()`.
2. **Class registrations are split**:
   - Rust modules handle: `uuid`, `locale`, `certificate`, `ecdsa_impl`
   - C++ wrappers handle: `File`, `BuildVersion`, `PackageInfo`, `PackageManager`, `Context`, `HardwareInformation`, `NetworkMonitor`, `MainActivity`, `JellyBeanDeviceManager`, `PlayIntegrity`
   - Baron C++ still registers: `Store`, `FMOD`, `XboxLive`, `HttpClient*`, `WebView`, `AssetManager`, `AudioDevice`, etc.
3. **`main_activity.cpp` still compiled** — its MainActivity methods are registered with libjnivm-sys via `jnivm_class_wrappers.cpp`, but the C++ class is still needed for Baron registration.
4. **`store.cpp` still compiled** — store classes are registered with Baron, not yet with libjnivm-sys.
5. **Startup orchestration is in Rust** — `main.rs` calls `jni_support::jni_support_start_game()` which handles both VMs.

## Porting Pattern

For each C++ JNI class:

1. Keep FakeJni registrations in `jni_support.cpp` **unchanged** (Baron still active for game dispatch)
2. Add `.cpp` to `excluded_jni` in `build.rs`
3. If the C++ class is also registered with Baron, create a `_stub.cpp` with minimal method bodies
4. Write the real implementation in Rust (register with libjnivm-sys)
5. Register the Rust implementation via `RegisterNatives` on the libjnivm-sys VM

Example: `signature.cpp` + `ecdsa.cpp`
- `signature_stub.cpp` (11 lines): `initVerify` → no-op, `verify` → true, `getInstance` → dummy
- `ecdsa_impl` Rust module: full secp256r1 via `p256` crate
- Both Baron and libjnivm-sys have registrations for these classes
- The Rust module registers with libjnivm-sys; Baron registrations remain in C++ files (unused by game dispatch)

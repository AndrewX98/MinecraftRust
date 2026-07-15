# Minecraft PE "Storage Full" Bug

## Symptom

Minecraft Launcher 1.26.32.2 displays:

```
Storage is full. Delete data from Settings to enter a world.
0 Bytes used out of 0 Bytes
```

Despite having 230GB+ of free disk space. Older versions work fine.

## Root Cause

The native game binary (`libminecraftpe.so`) checks disk space by creating
`java.io.File` objects via JNI and calling `File.getTotalSpace()` and
`File.getUsableSpace()` on them. The launcher's fake `File` class only
implemented `getPath()` — it had no `getTotalSpace()` or `getUsableSpace()`
methods.

When the native code called these methods via JNI reflection, the VM returned
0 (the default for unimplemented methods). The game interpreted 0 bytes total
and 0 bytes usable as "storage full".

## How the Native Code Checks Storage

```
1. Call getStoragePath() or getExternalStoragePath() → get a path string
2. Create a java.io.File object with that path (via JNI)
3. Call file.getTotalSpace() and file.getUsableSpace() (via JNI reflection)
4. Compare usable space against internal thresholds (isStorageFull/isStorageLow)
```

Step 3 failed because the `File` class had no implementation for these methods.

## Why It Happened

The launcher uses a "fake JVM" (Baron VM / FakeJni) that emulates Android's
Java runtime. C++ classes are registered to implement Java classes like
`java.io.File`, `android.os.StatFs`, and `com.mojang.minecraftpe.MainActivity`.

The `File` class was originally a minimal stub — only `getPath()` was needed
for earlier Minecraft versions. Newer versions (1.26.32.2) added JNI calls to
`File.getTotalSpace()` and `File.getUsableSpace()`, which the stub didn't
implement.

## The Fix

Added `getTotalSpace()` and `getUsableSpace()` methods to the `File` class
that use `statvfs()` to query real disk space:

```cpp
FakeJni::JLong getTotalSpace() {
    struct statvfs stat;
    if (::statvfs(path.c_str(), &stat) == 0) {
        return (FakeJni::JLong)stat.f_blocks * stat.f_bsize;
    }
    return 1024LL * 1024LL * 1024LL * 1024LL; // fallback: 1TB
}
```

Also added a JNI-compatible constructor so the native code can create `File`
objects from Java strings.

## Files Changed

| File | Change |
|------|--------|
| `mcpelauncher-client/src/jni/java_types.h` | Added `File(JString)`, `getTotalSpace()`, `getUsableSpace()`, `getFreeSpace()`, `getAbsolutePath()`, `exists()` |
| `mcpelauncher-client/src/jni/jni_descriptors.cpp` | Registered new File constructor and methods |
| `mcpelauncher-client/src/jni/main_activity.h` | Fixed `getTotalSpace` signature to `(Ljava/lang/String;)J` |
| `mcpelauncher-client/src/jni/stat_fs.h` | New `android.os.StatFs` implementation (used by DEX, not main storage check) |
| `libc-shim/src/statvfs.cpp` | Added path rewriting for `statvfs` shim |

## Why Older Versions Worked

Older Minecraft versions only called `calculateAvailableDiskFreeSpace()` and
`getUsableSpace()` on `MainActivity` directly — both already returned hardcoded
1TB. Version 1.26.32.2 added the `File.getTotalSpace()` / `File.getUsableSpace()`
code path as an additional (or primary) storage check.

## Architecture Note

The launcher's fake JVM has no real DEX loading. All Java classes are C++
implementations registered via `vm.registerClass<T>()`. This means:

- C++ registered classes are the **only** implementations
- There's no DEX-vs-C++ conflict
- Missing methods return 0/null instead of throwing exceptions

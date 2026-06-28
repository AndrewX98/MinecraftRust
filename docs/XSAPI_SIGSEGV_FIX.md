# XSAPI SIGSEGV Fix — Baron VM Thread Safety

## Symptom

SIGSEGV ("Address boundary error") during startup, immediately after XSAPI registers its Java classes:

```
[HttpCallStaticGlue] Successfully registerered HttpCall methods
[XboxLiveAppConfig] Successfully registerered XboxLiveAppConfig methods
[XSAPI.Android] Successfully registerered HttpCall tcuiMethods
fish: terminated by signal SIGSEGV (Address boundary error)
```

## GDB Backtrace

The crash is on **Thread 17 "IO Thread(0)"** — an XSAPI background thread spawned during `JNI_OnLoad`:

```
#0  operator()<std::shared_ptr<jnivm::Method>&> at method.cpp:173
    return !m->_static && mid->name == m->name && mid->signature == m->signature && m->nativehandle;
    // m has use count 1453527712 (garbage) — shared_ptr is corrupted

#1  std::__find_if — __last cannot be read at address 0x28
    // cl->methods.end() iterator is corrupted

#3  findNonVirtualOverload (cl=0x555556a08c30, mid=0x555556a0f300)
#4  findVirtualOverload
#5  jnivm::MDispatchBase2<unsigned char>::CallMethod
#6  jnivm::MDispatchBase<unsigned char, _jobject*>::CallMethod
#7  0x00007fffb229095a  ← game code calling into Baron VM via JNI
```

## Root Cause

The Baron/FakeJni VM (`libjnivm`) is **not thread-safe** for concurrent access to `Class::methods` and `Class::fields` vectors.

XSAPI's `JNI_OnLoad` inside `libminecraftpe.so`:
1. Registers Java classes (HttpCallStaticGlue, XboxLiveAppConfig, etc.)
2. Spawns background IO and REST threads
3. These threads immediately make JNI calls through the Baron VM

The race condition:

| Thread | Operation | Lock held? |
|--------|-----------|------------|
| IO Thread (background) | `findNonVirtualOverload` iterates `cl->methods` | No |
| REST Thread (background) | `doRequestAsync` → `getClass().getMethod()` | No |
| Main thread (during `JNI_OnLoad`) | `GetMethodID` → `cl->methods.push_back()` | No |

When `push_back()` reallocates the vector while another thread is iterating it, the iterator is invalidated and the `shared_ptr<Method>` control blocks get corrupted, causing SIGSEGV.

### Specific unprotected code paths

1. **`method.cpp:168` — `findNonVirtualOverload`**: iterates `cl->methods` without `cl->mtx`
2. **`method.cpp:64` — `GetMethodID`**: pushes to `cur->methods` outside the `cur->mtx` lock scope (lock ends at line 37, push is at line 64)
3. **`hookManager.h:50,62` — `FunctionBase::install`**: iterates and pushes to `cl->methods` without any lock
4. **`hookManager.h:103,115` — `PropertyBase::install`**: iterates and pushes to `cl->fields` without any lock

Meanwhile, `RegisterNatives` (`vm.cpp:296`) and `UnregisterNatives` (`vm.cpp:319`) correctly hold `clazz->mtx`, but the dispatch path (`findNonVirtualOverload`) and the hook install path do not.

## Fix

Added `std::lock_guard<std::mutex> lock(cl->mtx)` to all unprotected read/write paths on `Class::methods` and `Class::fields`:

### 1. `method.cpp` — `findNonVirtualOverload`

```cpp
static Method* findNonVirtualOverload(Class*cl, Method*mid) {
    if(!cl) { return mid; }
+   std::lock_guard<std::mutex> lock(cl->mtx);
    auto res = std::find_if(cl->methods.begin(), cl->methods.end(), [mid](auto&& m) {
        return !m->_static && mid->name == m->name && mid->signature == m->signature && m->nativehandle;
    });
    ...
}
```

### 2. `method.cpp` — `GetMethodID` (lazy method creation)

Moved the `push_back` inside the lock scope and set fields before locking to minimize critical section:

```cpp
-   next = std::make_shared<Method>();
-   if(cur) {
-       cur->methods.push_back(next);
-   }
-   next->name = std::move(sname);
-   next->signature = std::move(ssig);
-   next->_static = isStatic;
+   next = std::make_shared<Method>();
+   next->name = std::move(sname);
+   next->signature = std::move(ssig);
+   next->_static = isStatic;
+   if(cur) {
+       std::lock_guard<std::mutex> lock(cur->mtx);
+       cur->methods.push_back(next);
+   }
```

### 3. `hookManager.h` — `FunctionBase::install`

Added `std::lock_guard<std::mutex> lock(cl->mtx)` around the `find_if` + `push_back` on `cl->methods`.

### 4. `hookManager.h` — `PropertyBase::install`

Added `std::lock_guard<std::mutex> lock(cl->mtx)` around the `find_if` + `push_back` on `cl->fields`.

## Files Changed

| File | Change |
|------|--------|
| `crates/client/include/libjnivm/src/jnivm/internal/method.cpp` | Lock `cl->mtx` in `findNonVirtualOverload`; lock `cur->mtx` around `methods.push_back` in `GetMethodID` |
| `crates/client/include/libjnivm/jnivm/hookManager.h` | Lock `cl->mtx` in `FunctionBase::install` (2 overloads) and `PropertyBase::install` (2 overloads) |

## Mutex Coverage After Fix

| Operation | Lock | File |
|-----------|------|------|
| `GetMethodID` — search `methods` | `cur->mtx` | method.cpp:27 |
| `GetMethodID` — push to `methods` | `cur->mtx` | method.cpp:68 |
| `GetFieldID` — search/push `fields` | `cl->mtx` | field.cpp:14 |
| `findNonVirtualOverload` — iterate `methods` | `cl->mtx` | method.cpp:171 |
| `RegisterNatives` — push to `methods` | `clazz->mtx` | vm.cpp:296 |
| `UnregisterNatives` — erase from `methods` | `clazz->mtx` | vm.cpp:319 |
| `FunctionBase::install` — search/push `methods` | `cl->mtx` | hookManager.h:50,71 |
| `PropertyBase::install` — search/push `fields` | `cl->mtx` | hookManager.h:103,124 |

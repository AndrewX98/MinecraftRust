/// Minimal stubs for libHttpClient.Android.so symbols.
/// Registered with the C++ bionic linker BEFORE game loading,
/// so the real libHttpClient.Android.so is never loaded from disk.
/// Return E_FAIL (0x80004005) or similar error codes to signal
/// that the HTTP client/Xbox Live functionality is unavailable.
/// The game should gracefully handle this and run offline.

#include <cstdint>
#include <cstdio>
#include <cstring>
#include <cstdlib>

extern "C" {

// HRESULT constants
static constexpr long S_OK = 0;
static constexpr long E_FAIL = 0x80004005l;

// Fake opaque handles for XTaskQueue only (never dereferenced by game code).
static uint64_t g_next_handle = 0xBEEF0000ull;
static uint64_t g_process_task_queue = 0;

static uint64_t alloc_handle() {
    return ++g_next_handle;
}

// ---- HC_CALL stub objects -------------------------------------------------
//
// Real libHttpClient HC_CALL handles are heap pointers. Minecraft online-audio
// code on the FMOD "Streaming Pool" thread stores the handle in a wrapper and
// performs a C++ virtual call:
//   mov (%wrapper), %rax   ; rax = HC_CALL handle
//   call *0x10(%rax)       ; treat handle as a vtable / fn-table
//
// Returning integer IDs (0x1001…) made rax=0x1003 and SIGSEGV'd. We allocate
// a real function table so that call is a safe no-op returning an empty
// shared_ptr (matches the observed sret ABI at the crash site).

struct StubHCCall {
    // First word is a pointer to the method table (C++ object layout) so that
    // both of these patterns are safe:
    //   call *0x10(handle)           // handle used as vtable
    //   mov (handle), %vt; call *0x10(%vt)  // handle used as object
    void** vptr;
    void* methods[32];
    uint32_t magic;
    uint32_t refcount;
};

static constexpr uint32_t kStubHCCallMagic = 0x4843434cu; // 'HCCL'

// ---- Immortal shared_ptr result for online-audio virtual calls ------------
// After HC_CALL-as-vtable dispatch, the game does:
//   auto sp = obj->vmethod(...);   // shared_ptr sret
//   sp->vtable[6](...);            // call *0x30(vtable)
// Returning a null shared_ptr crashes on the second step. Hand out a static
// immortal object + control block instead (never freed).

struct StubSharedWeakCount {
    void** vptr;
    long shared_owners; // libc++: owners-1; 0 means one owner
    long shared_weak_owners;
};

struct StubResultObj {
    void** vptr;
    char pad[64];
};

static void stub_cb_noop(void*) {}
static void stub_cb_on_zero_shared(void*) {
    // Immortal — do not free.
}
static void stub_cb_on_zero_weak(void*) {}

// __shared_weak_count vtable (Itanium): dtor, deleting dtor, on_zero_shared, on_zero_weak
static void* g_stub_cb_vtable[] = {
    reinterpret_cast<void*>(&stub_cb_noop),
    reinterpret_cast<void*>(&stub_cb_noop),
    reinterpret_cast<void*>(&stub_cb_on_zero_shared),
    reinterpret_cast<void*>(&stub_cb_on_zero_weak),
};

// Result object methods — slot 0x30 / 8 = index 6 is called after shared_ptr return.
static void stub_result_method(void* /*self*/, void* /*arg*/) {
    // Online-audio / HTTP completion callback — no-op offline.
}
static void* g_stub_result_vtable[16] = {
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method), // +0x30
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
    reinterpret_cast<void*>(&stub_result_method),
};

static StubResultObj g_stub_result = {g_stub_result_vtable, {}};
// High owner count so release paths never destroy the immortal object.
static StubSharedWeakCount g_stub_control = {g_stub_cb_vtable, 0x1000000, 0x1000000};

// Virtual method ABI seen at crash: hidden sret shared_ptr in rdi, this in rsi.
static void stub_hc_vmethod_empty_shared(void* sret, void* /*self*/, void* /*arg*/) {
    if (!sret) {
        return;
    }
    auto* sp = static_cast<void**>(sret);
    sp[0] = &g_stub_result;
    sp[1] = &g_stub_control;
}

// Generic safe stubs for any other slot the game might hit.
static long stub_hc_vmethod_efail(void*, void*, void*, void*) {
    return E_FAIL;
}

static StubHCCall* stub_hc_call_new() {
    auto* c = static_cast<StubHCCall*>(std::calloc(1, sizeof(StubHCCall)));
    if (!c) {
        return nullptr;
    }
    for (int i = 0; i < 32; i++) {
        c->methods[i] = reinterpret_cast<void*>(&stub_hc_vmethod_empty_shared);
    }
    // Also plant efail variants in a few slots in case non-sret methods are used.
    c->methods[3] = reinterpret_cast<void*>(&stub_hc_vmethod_efail);
    c->methods[4] = reinterpret_cast<void*>(&stub_hc_vmethod_efail);
    c->vptr = c->methods;
    c->magic = kStubHCCallMagic;
    c->refcount = 1;
    return c;
}

static void stub_hc_call_free(uint64_t handle) {
    if (!handle) {
        return;
    }
    auto* c = reinterpret_cast<StubHCCall*>(handle);
    if (c->magic != kStubHCCallMagic) {
        // Might be a vtable-only pointer (methods array) from an older path.
        return;
    }
    if (--c->refcount == 0) {
        c->magic = 0;
        std::free(c);
    }
}

// ---- HC (HTTP Client) symbols ----

// Prefer E_FAIL: if XalInitialize is not patched, failing HC hard-fails
// with Xal::Exception rather than continuing into half-broken XAL state
// (SIGSEGV). CorePatches patches XalInitialize → S_OK so this is unused
// on the normal startup path.
long HCInitialize(uint64_t, void*) {
    return E_FAIL;
}

void HCCleanupAsync(uint64_t) {
}

long HCMemSetFunctions(void*, void*, void*, void*) {
    return S_OK;
}

long HCAddCallRoutedHandler(void*, void*, void*) {
    return 0;
}

long HCRemoveCallRoutedHandler(void*, void*) {
    return 0;
}

long HCSettingsGetTraceLevel(void*, uint32_t*) {
    return E_FAIL;
}

void HCSettingsSetTraceLevel(uint32_t) {
}

long HCTraceInit(void*, void*) {
    return S_OK;
}

void HCTraceCleanup() {
}

void HCTraceImplMessage(uint32_t, const char*, const char*) {
}

uint64_t HCTraceImplScopeId(const char*) {
    return 0;
}

void HCTraceSetPlatformCallbacks(void*, void*, void*) {
}

long HCHttpCallCreate(uint64_t, uint64_t* out) {
    // Real HC_CALL handles are heap object pointers. Integer IDs (old stub)
    // became fake vtables and crashed Streaming Pool (rax=0x1003).
    StubHCCall* call = stub_hc_call_new();
    if (!call) {
        if (out) {
            *out = 0;
        }
        return E_FAIL;
    }
    if (out) {
        *out = reinterpret_cast<uint64_t>(call);
    }
    fprintf(stderr, "=== HC stub: HCHttpCallCreate -> %p (stub object) ===\n", (void*)call);
    return S_OK;
}

long HCHttpCallDuplicateHandle(uint64_t handle, uint64_t* out) {
    if (!handle) {
        if (out) {
            *out = 0;
        }
        return E_FAIL;
    }
    auto* c = reinterpret_cast<StubHCCall*>(handle);
    if (c->magic == kStubHCCallMagic) {
        c->refcount++;
    }
    if (out) {
        *out = handle;
    }
    return S_OK;
}

long HCHttpCallCloseHandle(uint64_t handle) {
    stub_hc_call_free(handle);
    return S_OK;
}

uint64_t HCHttpCallGetId(uint64_t) {
    return 0;
}

long HCHttpCallGetRequestUrl(uint64_t, const char**) {
    return E_FAIL;
}

long HCHttpCallRequestGetUrl(uint64_t, const char**) {
    return E_FAIL;
}

long HCHttpCallRequestSetUrl(uint64_t, const char*) {
    return E_FAIL;
}

long HCHttpCallRequestSetHeader(uint64_t, const char*, const char*) {
    return E_FAIL;
}

long HCHttpCallRequestGetHeader(uint64_t, const char*, const char**) {
    return E_FAIL;
}

long HCHttpCallRequestGetNumHeaders(uint64_t, uint32_t*) {
    return E_FAIL;
}

long HCHttpCallRequestGetHeaderAtIndex(uint64_t, uint32_t, const char**, const char**) {
    return E_FAIL;
}

long HCHttpCallRequestSetRequestBodyBytes(uint64_t, const uint8_t*, uint32_t) {
    return E_FAIL;
}

long HCHttpCallRequestGetRequestBodyBytes(uint64_t, const uint8_t**, uint32_t*) {
    return E_FAIL;
}

long HCHttpCallRequestSetRequestBodyString(uint64_t, const char*) {
    return E_FAIL;
}

long HCHttpCallRequestSetRequestBodyReadFunction(uint64_t, void*, void*) {
    return E_FAIL;
}

long HCHttpCallRequestSetRetryAllowed(uint64_t, uint8_t) {
    return E_FAIL;
}

long HCHttpCallRequestGetRetryAllowed(uint64_t, uint8_t*) {
    return E_FAIL;
}

long HCHttpCallRequestSetRetryCacheId(uint64_t, const char*) {
    return E_FAIL;
}

long HCHttpCallRequestGetRetryCacheId(uint64_t, const char**) {
    return E_FAIL;
}

long HCHttpCallRequestSetRetryDelay(uint64_t, uint32_t) {
    return E_FAIL;
}

long HCHttpCallRequestGetRetryDelay(uint64_t, uint32_t*) {
    return E_FAIL;
}

long HCHttpCallRequestSetTimeout(uint64_t, uint32_t) {
    return E_FAIL;
}

long HCHttpCallRequestGetTimeout(uint64_t, uint32_t*) {
    return E_FAIL;
}

long HCHttpCallRequestSetTimeoutWindow(uint64_t, uint32_t) {
    return E_FAIL;
}

long HCHttpCallRequestGetTimeoutWindow(uint64_t, uint32_t*) {
    return E_FAIL;
}

long HCHttpCallPerformAsync(uint64_t, uint64_t) {
    return E_FAIL;
}

long HCHttpCallSetTracing(uint64_t, uint64_t) {
    return E_FAIL;
}

long HCHttpCallResponseGetStatusCode(uint64_t, uint32_t*) {
    return E_FAIL;
}

long HCHttpCallResponseGetNetworkErrorCode(uint64_t, uint32_t*, const char**) {
    return E_FAIL;
}

long HCHttpCallResponseGetPlatformNetworkErrorMessage(uint64_t, const char**) {
    return E_FAIL;
}

long HCHttpCallResponseGetNumHeaders(uint64_t, uint32_t*) {
    return E_FAIL;
}

long HCHttpCallResponseGetHeaderAtIndex(uint64_t, uint32_t, const char**, const char**) {
    return E_FAIL;
}

long HCHttpCallResponseGetHeader(uint64_t, const char*, const char**) {
    return E_FAIL;
}

long HCHttpCallResponseGetResponseBodyBytesSize(uint64_t, uint32_t*) {
    return E_FAIL;
}

long HCHttpCallResponseGetResponseBodyBytes(uint64_t, uint8_t*, uint32_t, uint32_t*, bool) {
    return E_FAIL;
}

long HCHttpCallResponseGetResponseString(uint64_t, const char**) {
    return E_FAIL;
}

long HCHttpCallResponseSetResponseBodyWriteFunction(uint64_t, void*, void*) {
    return E_FAIL;
}

long HCWebSocketCreate(uint64_t, uint64_t, uint64_t*) {
    return E_FAIL;
}

long HCWebSocketConnectAsync(uint64_t, const char*, uint64_t, void*, const char* const*, uint32_t) {
    return E_FAIL;
}

long HCWebSocketSendMessageAsync(uint64_t, const char*, uint64_t) {
    return E_FAIL;
}

long HCWebSocketDisconnect(uint64_t) {
    return S_OK;
}

long HCWebSocketCloseHandle(uint64_t) {
    return S_OK;
}

long HCWebSocketDuplicateHandle(uint64_t, uint64_t*) {
    return E_FAIL;
}

long HCWebSocketGetEventFunctions(uint64_t, void**, void**, void**) {
    return E_FAIL;
}

long HCWebSocketSetHeader(uint64_t, const char*, const char*) {
    return E_FAIL;
}

long HCWebSocketSetPingInterval(uint64_t, uint32_t) {
    return E_FAIL;
}

long HCGetWebSocketConnectResult(uint64_t, uint64_t*, uint64_t*) {
    return E_FAIL;
}

long HCGetWebSocketSendMessageResult(uint64_t, uint64_t, uint64_t*) {
    return E_FAIL;
}

// ---- XTaskQueue symbols ----

long XTaskQueueCreate(uint32_t, uint32_t, uint64_t* out) {
    if (out) {
        *out = alloc_handle();
    }
    return S_OK;
}

long XTaskQueueCreateComposite(uint64_t, uint64_t, uint64_t* out) {
    if (out) {
        *out = alloc_handle();
    }
    return S_OK;
}

long XTaskQueueDuplicateHandle(uint64_t handle, uint64_t* out) {
    if (out) {
        *out = handle ? handle : alloc_handle();
    }
    return S_OK;
}

long XTaskQueueCloseHandle(uint64_t) {
    return S_OK;
}

long XTaskQueueTerminate(uint64_t, bool, void*, void*) {
    return S_OK;
}

void XTaskQueueSetCurrentProcessTaskQueue(uint64_t queue) {
    g_process_task_queue = queue;
}

long XTaskQueueGetCurrentProcessTaskQueue(uint64_t* out) {
    if (out) {
        if (!g_process_task_queue) {
            g_process_task_queue = alloc_handle();
        }
        *out = g_process_task_queue;
    }
    return S_OK;
}

long XTaskQueueDispatch(uint64_t, uint32_t, uint64_t) {
    return 0; // false: nothing to dispatch
}

long XTaskQueueGetPort(uint64_t, uint32_t, uint64_t* out) {
    if (out) {
        *out = alloc_handle();
    }
    return S_OK;
}

long XTaskQueueRegisterMonitor(uint64_t, void*, void*, uint64_t* out) {
    if (out) {
        *out = alloc_handle();
    }
    return S_OK;
}

long XTaskQueueUnregisterMonitor(uint64_t, uint64_t) {
    return S_OK;
}

long XTaskQueueSubmitDelayedCallback(uint64_t, uint64_t, void*, void*, uint64_t* out) {
    if (out) {
        *out = alloc_handle();
    }
    // Accept the submit but never run the callback (offline stub).
    return S_OK;
}

// ---- XAsync symbols ----

long XAsyncBegin(uint64_t, void*, void*, const char*) {
    return E_FAIL;
}

long XAsyncCancel(uint64_t) {
    return S_OK;
}

long XAsyncComplete(uint64_t, long, uint32_t) {
    return S_OK;
}

long XAsyncGetStatus(uint64_t, bool) {
    return E_FAIL;
}

long XAsyncGetResult(uint64_t, void*, uint32_t, void*, uint32_t*) {
    return E_FAIL;
}

long XAsyncGetResultSize(uint64_t, uint32_t*) {
    return E_FAIL;
}

long XAsyncSchedule(uint64_t, uint64_t) {
    return E_FAIL;
}

} // extern "C"

// Registration function: adds all stubs to the C++ bionic linker as a "loaded" library,
// so the real libHttpClient.Android.so is never loaded from disk during DT_NEEDED resolution.
// This mirrors the FMOD stub pattern in main.rs.
#include <cstdlib>
#include <cstdio>
#include <cstring>
#include <vector>
#include <unordered_map>
#include <string>
#include <mcpelauncher/linker.h>

extern "C" size_t linker_load_library_rust(const char* name, const char* const* keys, void* const* vals, size_t len);

extern "C" void http_client_register_stubs() {
    std::unordered_map<std::string, void*> syms;

    struct SymEntry { const char* name; void* func; };
#define SYM(name) {#name, (void*)name}
    SymEntry table[] = {
        SYM(HCInitialize),
        SYM(HCCleanupAsync),
        SYM(HCMemSetFunctions),
        SYM(HCAddCallRoutedHandler),
        SYM(HCRemoveCallRoutedHandler),
        SYM(HCSettingsGetTraceLevel),
        SYM(HCSettingsSetTraceLevel),
        SYM(HCTraceInit),
        SYM(HCTraceCleanup),
        SYM(HCTraceImplMessage),
        SYM(HCTraceImplScopeId),
        SYM(HCTraceSetPlatformCallbacks),
        SYM(HCHttpCallCreate),
        SYM(HCHttpCallDuplicateHandle),
        SYM(HCHttpCallCloseHandle),
        SYM(HCHttpCallGetId),
        SYM(HCHttpCallGetRequestUrl),
        SYM(HCHttpCallRequestGetUrl),
        SYM(HCHttpCallRequestSetUrl),
        SYM(HCHttpCallRequestSetHeader),
        SYM(HCHttpCallRequestGetHeader),
        SYM(HCHttpCallRequestGetNumHeaders),
        SYM(HCHttpCallRequestGetHeaderAtIndex),
        SYM(HCHttpCallRequestSetRequestBodyBytes),
        SYM(HCHttpCallRequestGetRequestBodyBytes),
        SYM(HCHttpCallRequestSetRequestBodyString),
        SYM(HCHttpCallRequestSetRequestBodyReadFunction),
        SYM(HCHttpCallRequestSetRetryAllowed),
        SYM(HCHttpCallRequestGetRetryAllowed),
        SYM(HCHttpCallRequestSetRetryCacheId),
        SYM(HCHttpCallRequestGetRetryCacheId),
        SYM(HCHttpCallRequestSetRetryDelay),
        SYM(HCHttpCallRequestGetRetryDelay),
        SYM(HCHttpCallRequestSetTimeout),
        SYM(HCHttpCallRequestGetTimeout),
        SYM(HCHttpCallRequestSetTimeoutWindow),
        SYM(HCHttpCallRequestGetTimeoutWindow),
        SYM(HCHttpCallPerformAsync),
        SYM(HCHttpCallSetTracing),
        SYM(HCHttpCallResponseGetStatusCode),
        SYM(HCHttpCallResponseGetNetworkErrorCode),
        SYM(HCHttpCallResponseGetPlatformNetworkErrorMessage),
        SYM(HCHttpCallResponseGetNumHeaders),
        SYM(HCHttpCallResponseGetHeaderAtIndex),
        SYM(HCHttpCallResponseGetHeader),
        SYM(HCHttpCallResponseGetResponseBodyBytesSize),
        SYM(HCHttpCallResponseGetResponseBodyBytes),
        SYM(HCHttpCallResponseGetResponseString),
        SYM(HCHttpCallResponseSetResponseBodyWriteFunction),
        SYM(HCWebSocketCreate),
        SYM(HCWebSocketConnectAsync),
        SYM(HCWebSocketSendMessageAsync),
        SYM(HCWebSocketDisconnect),
        SYM(HCWebSocketCloseHandle),
        SYM(HCWebSocketDuplicateHandle),
        SYM(HCWebSocketGetEventFunctions),
        SYM(HCWebSocketSetHeader),
        SYM(HCWebSocketSetPingInterval),
        SYM(HCGetWebSocketConnectResult),
        SYM(HCGetWebSocketSendMessageResult),
        SYM(XTaskQueueCreate),
        SYM(XTaskQueueCreateComposite),
        SYM(XTaskQueueDuplicateHandle),
        SYM(XTaskQueueCloseHandle),
        SYM(XTaskQueueTerminate),
        SYM(XTaskQueueSetCurrentProcessTaskQueue),
        SYM(XTaskQueueGetCurrentProcessTaskQueue),
        SYM(XTaskQueueDispatch),
        SYM(XTaskQueueGetPort),
        SYM(XTaskQueueRegisterMonitor),
        SYM(XTaskQueueUnregisterMonitor),
        SYM(XTaskQueueSubmitDelayedCallback),
        SYM(XAsyncBegin),
        SYM(XAsyncCancel),
        SYM(XAsyncComplete),
        SYM(XAsyncGetStatus),
        SYM(XAsyncGetResult),
        SYM(XAsyncGetResultSize),
        SYM(XAsyncSchedule),
    };
#undef SYM

    for (auto& e : table) {
        syms[e.name] = e.func;
    }

    char* name_stable = (char*)malloc(strlen("libHttpClient.Android.so") + 1);
    strcpy(name_stable, "libHttpClient.Android.so");
    void* handle = linker::load_library(name_stable, syms);
    // Mirror to Rust linker state
    {
        size_t n = syms.size();
        std::vector<const char*> keys(n);
        std::vector<void*> vals(n);
        size_t i = 0;
        for (auto& [k, v] : syms) {
            keys[i] = k.c_str();
            vals[i] = v;
            i++;
        }
        linker_load_library_rust(name_stable, keys.data(), vals.data(), n);
    }
    fprintf(stderr, "LAUNCHER: http_client_register_stubs: registered %zu symbols, handle=%p\n",
            sizeof(table) / sizeof(table[0]), handle);
}

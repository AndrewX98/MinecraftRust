/// Minimal stubs for libHttpClient.Android.so symbols.
/// Registered with the C++ bionic linker BEFORE game loading,
/// so the real libHttpClient.Android.so is never loaded from disk.
/// Return E_FAIL (0x80004005) or similar error codes to signal
/// that the HTTP client/Xbox Live functionality is unavailable.
/// The game should gracefully handle this and run offline.

#include <cstdint>

extern "C" {

// HRESULT constants
static constexpr long S_OK = 0;
static constexpr long E_FAIL = 0x80004005l;

// ---- HC (HTTP Client) symbols ----

long HCInitialize(uint64_t, void*) {
    return E_FAIL;
}

void HCCleanupAsync(uint64_t) {
}

long HCMemSetFunctions(void*, void*, void*, void*) {
    return E_FAIL;
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
    return E_FAIL;
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

long HCHttpCallCreate(uint64_t, uint64_t*) {
    return E_FAIL;
}

long HCHttpCallDuplicateHandle(uint64_t, uint64_t*) {
    return E_FAIL;
}

long HCHttpCallCloseHandle(uint64_t) {
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

long XTaskQueueCreate(uint32_t, uint32_t, uint64_t*) {
    return E_FAIL;
}

long XTaskQueueCreateComposite(uint64_t, uint64_t, uint64_t*) {
    return E_FAIL;
}

long XTaskQueueDuplicateHandle(uint64_t, uint64_t*) {
    return E_FAIL;
}

long XTaskQueueCloseHandle(uint64_t) {
    return S_OK;
}

long XTaskQueueTerminate(uint64_t, bool, void*, void*) {
    return E_FAIL;
}

void XTaskQueueSetCurrentProcessTaskQueue(uint64_t) {
}

long XTaskQueueGetCurrentProcessTaskQueue(uint64_t*) {
    return E_FAIL;
}

long XTaskQueueDispatch(uint64_t, uint32_t, uint64_t) {
    return 0;
}

long XTaskQueueGetPort(uint64_t, uint32_t, uint64_t*) {
    return E_FAIL;
}

long XTaskQueueRegisterMonitor(uint64_t, void*, void*, uint64_t*) {
    return E_FAIL;
}

long XTaskQueueUnregisterMonitor(uint64_t, uint64_t) {
    return E_FAIL;
}

long XTaskQueueSubmitDelayedCallback(uint64_t, uint64_t, void*, void*, uint64_t*) {
    return E_FAIL;
}

// ---- XAsync symbols ----

long XAsyncBegin(uint64_t, void*, void*, const char*) {
    return E_FAIL;
}

long XAsyncCancel(uint64_t) {
    return E_FAIL;
}

long XAsyncComplete(uint64_t, long, uint32_t) {
    return E_FAIL;
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

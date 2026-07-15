/// Stub replacing jni/xbox_live.cpp for the Rust build.
/// The real implementation lives in Rust (crates/client/src/jni/xbox_live.rs),
/// registered with the libjnivm-sys VM which serves all game JNI dispatch.
/// These FakeJni bodies remain only because jni_descriptors.cpp references the
/// method pointers and jni_support.cpp registers the classes with the Baron VM
/// (XSAPI background threads attached via ga->vm may still dispatch here).
/// Behavior mirrors the previous xbox_live.cpp + xbox_live_helper_stub.cpp
/// combination: auth always fails immediately, CLL events are dropped.

#include <FileUtil.h>
#include <mcpelauncher/path_helper.h>
#include <log.h>
#include "jni/xbox_live.h"

std::shared_ptr<FakeJni::JString> XboxInterop::getLocalStoragePath(std::shared_ptr<Context> context) {
    return std::make_shared<FakeJni::JString>(PathHelper::getPrimaryDataDirectory());
}

std::shared_ptr<FakeJni::JString> XboxInterop::readConfigFile(std::shared_ptr<Context> context) {
    std::string str;
    if(!FileUtil::readFile(PathHelper::findGameFile("assets/xboxservices.config"), str))
        str = "{}";
    return std::make_shared<FakeJni::JString>(str);
}

std::shared_ptr<FakeJni::JString> XboxInterop::getLocale() {
    return std::make_shared<FakeJni::JString>("en");
}

void XboxInterop::invokeMSA(std::shared_ptr<Context> context, FakeJni::JInt requestCode, FakeJni::JBoolean isProd,
                            std::shared_ptr<FakeJni::JString> cid) {
    Log::info("XboxInterop", "InvokeMSA (stub): requestCode=%i cid=%s", requestCode, cid->asStdString().c_str());
    FakeJni::Jvm const *vm = &FakeJni::JniEnv::getCurrentEnv()->getVM();

    if(requestCode == 1) {  // Silent sign in — always fails (no MSA daemon)
        ticketCallback(*vm, "", requestCode, TICKET_UNKNOWN_ERROR, "Xbox Live not available (stub)");
    } else if(requestCode == 6) {  // Sign out
        signOutCallback();
    } else {
        Log::error("XboxInterop", "Unsupported requestCode %i (stub)", requestCode);
    }
}

void XboxInterop::invokeAuthFlow(FakeJni::JLong userPtr, std::shared_ptr<Activity> activity, FakeJni::JBoolean isProd,
                                 std::shared_ptr<FakeJni::JString> signInText) {
    Log::info("XboxInterop", "InvokeAuthFlow (stub): always fails");
    FakeJni::Jvm const *vm = &FakeJni::JniEnv::getCurrentEnv()->getVM();
    authFlowCallback(*vm, userPtr, AUTH_FLOW_ERROR, "");
}

void XboxInterop::initCLL(std::shared_ptr<Context> arg0, std::shared_ptr<FakeJni::JString> arg1) {
    Log::warn("XboxInterop", "initCLL (stub): CLL telemetry not available");
}

void XboxInterop::logCLL(std::shared_ptr<FakeJni::JString> ticket, std::shared_ptr<FakeJni::JString> name, std::shared_ptr<FakeJni::JString> data) {
    Log::warn("XboxInterop", "logCLL (stub): event dropped");
}

void XboxInterop::ticketCallback(FakeJni::Jvm const &vm, std::string const &ticket, int requestCode, int errorCode,
                                 std::string const &error) {
    FakeJni::LocalFrame env(vm);
    auto callback = getDescriptor()->getMethod("(Ljava/lang/String;IILjava/lang/String;)V", "ticket_callback");
    auto ticketRef = env.getJniEnv().createLocalReference(std::make_shared<FakeJni::JString>(ticket));
    auto errorStrRef = env.getJniEnv().createLocalReference(std::make_shared<FakeJni::JString>(error));
    callback->invoke(env.getJniEnv(), getDescriptor().get(), ticketRef, requestCode, errorCode, errorStrRef);
}

void XboxInterop::authFlowCallback(FakeJni::Jvm const &vm, FakeJni::JLong userPtr, int status, std::string const &cid) {
    FakeJni::LocalFrame env(vm);
    auto callback = getDescriptor()->getMethod("(JILjava/lang/String;)V", "auth_flow_callback");
    auto cidRef = env.getJniEnv().createLocalReference(std::make_shared<FakeJni::JString>(cid));
    callback->invoke(env.getJniEnv(), getDescriptor().get(), userPtr, status, cidRef);
}

void XboxInterop::signOutCallback() {
    FakeJni::LocalFrame env;
    auto callback = getDescriptor()->getMethod("()V", "sign_out_callback");
    callback->invoke(env.getJniEnv(), getDescriptor().get());
}

// Auth never succeeds with the stubbed MSA path, so the XBLogin chain is dead
// code — but the descriptors reference these methods, so bodies must exist.
void XboxLoginCallback::onLogin(FakeJni::JLong nativePtr, FakeJni::JBoolean newAccount) {
    Log::warn("XboxLoginCallback", "onLogin (stub): unreachable");
}

void XboxLoginCallback::onSuccess() {
    XboxInterop::authFlowCallback(jvm, userPtr, XboxInterop::AUTH_FLOW_OK, cid);
}

void XboxLoginCallback::onError(int httpStatus, int status, std::shared_ptr<FakeJni::JString> message) {
    XboxInterop::authFlowCallback(jvm, userPtr, XboxInterop::AUTH_FLOW_ERROR, "");
}

std::shared_ptr<FakeJni::JString> XboxLocalStorage::getPath(std::shared_ptr<Context> context) {
    return XboxInterop::getLocalStoragePath(context);
}

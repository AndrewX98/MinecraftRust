/// Stub replacing xbox_live_helper.cpp for the Rust build.
/// Xbox Live authentication is not needed for offline play.
/// All methods are no-ops or return empty values.

#include "xbox_live_helper.h"
#include <log.h>

std::string const XboxLiveHelper::MSA_CLIENT_ID = "android-app://com.mojang.minecraftpe.H62DKCBHJP6WXXIV7RBFOGOL4NAK4E6Y";
std::string const XboxLiveHelper::MSA_COBRAND_ID = "90023";

XboxLiveHelper XboxLiveHelper::instance;

std::string XboxLiveHelper::findMsa() {
    return std::string();
}

XboxLiveHelper::XboxLiveHelper() : launcher(""), triedToCreateClient(false) {
}

msa::client::ServiceClient* XboxLiveHelper::getMsaClientOrNull() {
    return nullptr;
}

msa::client::ServiceClient& XboxLiveHelper::getMsaClient() {
    throw std::runtime_error("MSA daemon not available (stub)");
}

void XboxLiveHelper::invokeMsaAuthFlow(
    std::function<void(std::string const& cid, std::string const& binaryToken)> success_cb,
    std::function<void(simpleipc::rpc_error_code, std::string const&)> error_cb) {
    if(error_cb)
        error_cb(simpleipc::rpc_error_codes::connection_closed, "Xbox Live not available (stub)");
}

simpleipc::client::rpc_call<std::shared_ptr<msa::client::Token>> XboxLiveHelper::requestXblToken(std::string const& cid, bool silent) {
    throw std::runtime_error("Xbox Live not available (stub)");
}

void XboxLiveHelper::requestXblToken(std::string const& cid, bool silent,
    std::function<void(std::string const&, std::string const&)> success_cb,
    std::function<void(simpleipc::rpc_error_code, std::string const&)> error_cb) {
    if(error_cb)
        error_cb(simpleipc::rpc_error_codes::connection_closed, "Xbox Live not available (stub)");
}

void XboxLiveHelper::initCll(std::string const& cid) {
    Log::warn("XboxLiveHelper", "CLL telemetry not available (stub)");
}

std::string XboxLiveHelper::getCllMsaToken(std::string const& cid) {
    return std::string();
}

void XboxLiveHelper::setJvm(FakeJni::Jvm* vm) {
    this->vm = vm;
}

std::string XboxLiveHelper::getCllXToken(bool refresh) {
    return std::string();
}

std::string XboxLiveHelper::getCllXTicket(std::string const& xuid) {
    return std::string();
}

void XboxLiveHelper::logCll(cll::Event const& event) {
    Log::warn("XboxLiveHelper", "CLL event dropped (stub)");
}

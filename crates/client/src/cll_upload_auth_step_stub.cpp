/// Stub replacing cll_upload_auth_step.cpp for the Rust build.
/// CLL telemetry uploads are not functional in the Rust build — provide
/// no-op implementations to satisfy the class interface.

#include "cll_upload_auth_step.h"
#include <cll/event_batch.h>

void CllUploadAuthStep::setAccount(std::string const&) {
    // no-op
}

void CllUploadAuthStep::refreshTokens(bool) {
    // no-op
}

void CllUploadAuthStep::onRequest(cll::EventUploadRequest&) {
    // no-op
}

bool CllUploadAuthStep::onAuthenticationFailed() {
    return true; // retry
}

/// Stub replacing xal_webview_factory.cpp for the Rust build.
/// XAL webview is never used in the Rust build path — throw at runtime.

#include "xal_webview_factory.h"
#include <stdexcept>

std::unique_ptr<XalWebView> XalWebViewFactory::createXalWebView() {
    throw std::runtime_error("No XalWebView implementation available (Rust build)");
}

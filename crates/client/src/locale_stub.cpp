#include "jni/locale.h"

Locale::Locale(std::locale locale) : l(locale) {}

std::shared_ptr<Locale> Locale::getDefault() {
    // Use the classic locale so host libc++ never tries to construct
    // collate_byname for Android-style names like "en.UTF-8".
    return std::make_shared<Locale>(std::locale::classic());
}

std::shared_ptr<FakeJni::JString> Locale::toString() {
    // Game code often builds C++ locale names as language + ".UTF-8".
    // Return a form that maps cleanly (and that our newlocale shim accepts).
    return std::make_shared<FakeJni::JString>("en_US");
}

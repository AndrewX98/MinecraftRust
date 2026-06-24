#include "jni/locale.h"

Locale::Locale(std::locale locale) : l(locale) {}

std::shared_ptr<Locale> Locale::getDefault() {
    return std::make_shared<Locale>(std::locale());
}

std::shared_ptr<FakeJni::JString> Locale::toString() {
    return std::make_shared<FakeJni::JString>(l.name());
}

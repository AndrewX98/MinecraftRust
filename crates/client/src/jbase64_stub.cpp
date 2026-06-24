#include "jni/jbase64.h"
#include <cstring>
#include <cstdlib>

extern "C" {
    void* jbase64_decode_rust(const char* data, int len, int* out_len);
}

std::shared_ptr<FakeJni::JByteArray> JBase64::decode(std::shared_ptr<FakeJni::JString> value, int flags) {
    (void)flags;
    auto str = value->asStdString();
    int out_len;
    void* raw = jbase64_decode_rust(str.data(), (int)str.length(), &out_len);
    auto ret = std::make_shared<FakeJni::JByteArray>(out_len);
    if (raw) {
        memcpy(ret->getArray(), raw, out_len);
        free(raw);
    }
    return ret;
}

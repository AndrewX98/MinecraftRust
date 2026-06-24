#include "jni/securerandom.h"
#include <cstring>
#include <cstdlib>

extern "C" {
    void* securerandom_generate_bytes_rust(int bytes, int* out_len);
}

std::shared_ptr<FakeJni::JByteArray> SecureRandom::GenerateRandomBytes(int bytes) {
    int out_len;
    void* raw = securerandom_generate_bytes_rust(bytes, &out_len);
    auto ret = std::make_shared<FakeJni::JByteArray>(out_len);
    if (raw) {
        memcpy(ret->getArray(), raw, out_len);
        free(raw);
    }
    return ret;
}

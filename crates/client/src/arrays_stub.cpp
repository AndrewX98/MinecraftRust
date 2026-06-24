#include "jni/arrays.h"
#include <cstring>
#include <cstdlib>

extern "C" {
    void* arrays_copy_of_range_rust(const void* data, int offset, int len, int* out_len);
}

std::shared_ptr<FakeJni::JByteArray> Arrays::copyOfRange(std::shared_ptr<FakeJni::JByteArray> in, int i, int n) {
    int out_len = n - i;
    void* raw = arrays_copy_of_range_rust((const void*)in->getArray(), i, out_len, &out_len);
    auto ret = std::make_shared<FakeJni::JByteArray>(out_len);
    if (raw) {
        memcpy(ret->getArray(), raw, out_len);
        free(raw);
    }
    return ret;
}

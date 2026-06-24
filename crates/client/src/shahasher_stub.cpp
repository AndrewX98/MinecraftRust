#include "jni/shahasher.h"
#include <cstring>
#include <cstdlib>

extern "C" {
    void* shahasher_init_rust();
    void  shahasher_add_bytes_rust(void* ctx, const unsigned char* data, int len);
    void* shahasher_sign_hash_rust(void* ctx, int* out_len);
    void  shahasher_free_rust(void* ctx);
}

ShaHasher::ShaHasher() {
    mdctx = (EVP_MD_CTX*)shahasher_init_rust();
    if (!mdctx) throw std::runtime_error("ShaHasher: init failed");
}

ShaHasher::~ShaHasher() {
    shahasher_free_rust(mdctx);
}

void ShaHasher::AddBytes(std::shared_ptr<FakeJni::JByteArray> barray) {
    shahasher_add_bytes_rust(mdctx, (const unsigned char*)barray->getArray(), barray->getSize());
}

std::shared_ptr<FakeJni::JByteArray> ShaHasher::SignHash() {
    int md_len;
    void* raw = shahasher_sign_hash_rust(mdctx, &md_len);
    auto arr = std::make_shared<FakeJni::JByteArray>(md_len);
    if (raw) {
        memcpy(arr->getArray(), raw, md_len);
        free(raw);
    }
    return arr;
}

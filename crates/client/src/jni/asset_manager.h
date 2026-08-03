#pragma once

#include <fake-jni/fake-jni.h>

class AssetManager : public FakeJni::JObject {
public:
    DEFINE_CLASS_NAME("android/content/res/AssetManager")

    AssetManager();
};

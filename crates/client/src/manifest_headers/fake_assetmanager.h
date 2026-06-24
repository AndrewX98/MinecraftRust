#pragma once

#include <string>
#include <memory>
#include <unordered_map>
#include <utility>

struct AAssetManager;

struct FakeAssetManager {
    std::string rootDir;

    FakeAssetManager(std::string rootDir);

    static void initHybrisHooks(std::unordered_map<std::string, void *> &syms);

    static void setGlobalAssetManager(AAssetManager *mgr);
    static AAssetManager *getGlobalAssetManager();

    explicit operator AAssetManager *() const {
        return (AAssetManager *)this;
    }
};

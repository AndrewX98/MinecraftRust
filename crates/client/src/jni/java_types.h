#pragma once

#include <fake-jni/fake-jni.h>
#include <sys/statvfs.h>
#include <unistd.h>
#include <cstdlib>

class File : public FakeJni::JObject {
public:
    DEFINE_CLASS_NAME("java/io/File")

    std::string path;

    explicit File(std::string path) : path(std::move(path)) {
    }

    explicit File(std::shared_ptr<FakeJni::JString> path) : path(path->asStdString()) {
    }

    std::shared_ptr<FakeJni::JString> getPath() {
        return std::make_shared<FakeJni::JString>(path.c_str());
    }

    std::shared_ptr<FakeJni::JString> getAbsolutePath() {
        char* abs = realpath(path.c_str(), nullptr);
        if (abs) {
            auto result = std::make_shared<FakeJni::JString>(abs);
            free(abs);
            return result;
        }
        return getPath();
    }

    FakeJni::JBoolean exists() {
        return access(path.c_str(), F_OK) == 0 ? true : false;
    }

    FakeJni::JLong getTotalSpace() {
        struct statvfs stat;
        if (::statvfs(path.c_str(), &stat) == 0) {
            return (FakeJni::JLong)stat.f_blocks * stat.f_bsize;
        }
        return 1024LL * 1024LL * 1024LL * 1024LL;
    }

    FakeJni::JLong getUsableSpace() {
        struct statvfs stat;
        if (::statvfs(path.c_str(), &stat) == 0) {
            return (FakeJni::JLong)stat.f_bavail * stat.f_bsize;
        }
        return 1024LL * 1024LL * 1024LL * 1024LL;
    }

    FakeJni::JLong getFreeSpace() {
        struct statvfs stat;
        if (::statvfs(path.c_str(), &stat) == 0) {
            return (FakeJni::JLong)stat.f_bfree * stat.f_bsize;
        }
        return 1024LL * 1024LL * 1024LL * 1024LL;
    }
};

class ClassLoader : public FakeJni::JObject {
public:
    DEFINE_CLASS_NAME("java/lang/ClassLoader")

    static std::shared_ptr<ClassLoader> getInstance() {
        static std::shared_ptr<ClassLoader> instance(new ClassLoader);
        return instance;
    }

    std::shared_ptr<FakeJni::JClass> loadClass(std::shared_ptr<FakeJni::JString> str) {
        FakeJni::JniEnvContext context;
        return std::const_pointer_cast<FakeJni::JClass>(
            context.getJniEnv().getVM().findClass(str->asStdString().c_str()));
    }
};

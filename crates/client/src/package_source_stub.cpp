#include "jni/package_source.h"

NativePackageSourceListener::NativePackageSourceListener() {}

std::shared_ptr<PackageSource> PackageSourceFactory::createGooglePlayPackageSource(
    std::shared_ptr<FakeJni::JString>, std::shared_ptr<PackageSourceListener>) {
    return std::make_shared<PackageSource>();
}

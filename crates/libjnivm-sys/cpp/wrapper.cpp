#include <jni.h>
#include <jnivm.h>
#include <jnivm/vm.h>
#include <jnivm/env.h>
#include <memory>

extern "C" {

static std::shared_ptr<jnivm::VM> s_vm;

JavaVM* jnivm_create_vm() {
    s_vm = std::make_shared<jnivm::VM>();
    return s_vm->GetJavaVM();
}

void jnivm_destroy_vm(JavaVM* vm) {
    s_vm.reset();
}

JNIEnv* jnivm_get_env(JavaVM* vm) {
    auto realVm = jnivm::VM::FromJavaVM(vm);
    auto env = realVm->GetEnv();
    return env->GetJNIEnv();
}

jclass jnivm_find_class(JNIEnv* env, const char* name) {
    return env->FindClass(name);
}

jmethodID jnivm_get_method_id(JNIEnv* env, jclass clazz, const char* name, const char* sig) {
    return env->GetMethodID(clazz, name, sig);
}

jmethodID jnivm_get_static_method_id(JNIEnv* env, jclass clazz, const char* name, const char* sig) {
    return env->GetStaticMethodID(clazz, name, sig);
}

jfieldID jnivm_get_field_id(JNIEnv* env, jclass clazz, const char* name, const char* sig) {
    return env->GetFieldID(clazz, name, sig);
}

jfieldID jnivm_get_static_field_id(JNIEnv* env, jclass clazz, const char* name, const char* sig) {
    return env->GetStaticFieldID(clazz, name, sig);
}

void jnivm_register_natives(JNIEnv* env, jclass clazz, const JNINativeMethod* methods, jint count) {
    env->RegisterNatives(clazz, methods, count);
}

} // extern "C"

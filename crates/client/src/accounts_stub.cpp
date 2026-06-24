#include "jni/accounts.h"

std::shared_ptr<AccountManager> AccountManager::get(std::shared_ptr<Context>) {
    return std::make_shared<AccountManager>();
}

std::shared_ptr<FakeJni::JArray<Account>> AccountManager::getAccountsByType(std::shared_ptr<FakeJni::JString>) {
    return std::make_shared<FakeJni::JArray<Account>>(0);
}

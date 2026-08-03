#pragma once

#include <string>
#include <unordered_map>

class HybrisUtils {

private:
    static const char* TAG;

public:
    static void* loadLibraryOS(const char *name, std::string const &path, const char** symbols, std::unordered_map<std::string, void*> syms);
    static void* loadLibraryOS(const char *name, std::string const &path, const char** symbols);

    static void stubSymbols(const char *name, const char** symbols, void* stubfunc);

};
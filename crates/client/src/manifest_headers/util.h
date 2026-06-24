#pragma once

#include <string>
#include <algorithm>
#include <cstdlib>

static inline void ltrim(std::string &s) {
    s.erase(s.begin(), std::find_if(s.begin(), s.end(), [](int ch) {
                return !std::isspace(ch);
            }));
}

static inline void rtrim(std::string &s) {
    s.erase(std::find_if(s.rbegin(), s.rend(), [](int ch) {
                return !std::isspace(ch);
            }).base(),
            s.end());
}

static inline void trim(std::string &s) {
    ltrim(s);
    rtrim(s);
}

static inline bool ReadEnvFlag(const char *name, bool def = false) {
    auto val = getenv(name);
    if(!val) {
        return def;
    }
    std::string sval = val;
    return sval == "true" || sval == "1" || sval == "on";
}

static inline int ReadEnvInt(const char *name, int def = 0) {
    auto val = getenv(name);
    if(!val) {
        return def;
    }
    std::string sval = val;
    return std::stoi(sval);
}

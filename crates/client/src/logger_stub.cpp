#include <log.h>
#include <cstdio>
#include <cstdarg>

extern "C" {
    void mcpelauncher_log_vlog(int level, const char* tag, const char* text);
}

void Log::vlog(LogLevel level, const char* tag, const char* text, va_list args) {
    char buffer[4096];
    int len = vsnprintf(buffer, sizeof(buffer), text, args);
    if (len > (int)sizeof(buffer))
        len = (int)sizeof(buffer);
    while (len > 0 && (buffer[len - 1] == '\r' || buffer[len - 1] == '\n'))
        buffer[--len] = '\0';
    mcpelauncher_log_vlog((int)level, tag, buffer);
}

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include <unistd.h>

// Variadic shim wrappers — C handles va_start/va_end properly.
// Each is static (file-local) to avoid global symbol conflicts.
// Exposed via getter functions for the Rust symbol table.

static int shim_sscanf(const char *s, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vsscanf(s, fmt, ap);
    va_end(ap);
    return r;
}
void *get_shim_sscanf(void) { return (void *)shim_sscanf; }

static int shim_printf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vprintf(fmt, ap);
    va_end(ap);
    return r;
}
void *get_shim_printf(void) { return (void *)shim_printf; }

static int shim_sprintf(char *buf, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vsprintf(buf, fmt, ap);
    va_end(ap);
    return r;
}
void *get_shim_sprintf(void) { return (void *)shim_sprintf; }

static int shim_snprintf(char *buf, size_t size, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vsnprintf(buf, size, fmt, ap);
    va_end(ap);
    return r;
}
void *get_shim_snprintf(void) { return (void *)shim_snprintf; }

static int shim_asprintf(char **s, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vasprintf(s, fmt, ap);
    va_end(ap);
    return r;
}
void *get_shim_asprintf(void) { return (void *)shim_asprintf; }

static int shim___snprintf_chk(char *buf, size_t size, int flags, size_t dst_len, const char *fmt, ...) {
    (void)flags; (void)dst_len;
    va_list ap;
    va_start(ap, fmt);
    int r = vsnprintf(buf, size, fmt, ap);
    va_end(ap);
    return r;
}
void *get_shim___snprintf_chk(void) { return (void *)shim___snprintf_chk; }

static int shim_scanf(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vscanf(fmt, ap);
    va_end(ap);
    return r;
}
void *get_shim_scanf(void) { return (void *)shim_scanf; }

static int shim_swprintf(wchar_t *wcs, size_t maxlen, const wchar_t *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vswprintf(wcs, maxlen, fmt, ap);
    va_end(ap);
    return r;
}
void *get_shim_swprintf(void) { return (void *)shim_swprintf; }

static long shim_syscall(long number, ...) {
    va_list ap;
    va_start(ap, number);
    long a1 = va_arg(ap, long);
    long a2 = va_arg(ap, long);
    long a3 = va_arg(ap, long);
    long a4 = va_arg(ap, long);
    long a5 = va_arg(ap, long);
    long a6 = va_arg(ap, long);
    va_end(ap);
    return syscall(number, a1, a2, a3, a4, a5, a6);
}
void *get_shim_syscall(void) { return (void *)shim_syscall; }

// Must match Rust's BionicFile layout (LP64)
typedef struct {
    const char *_p;
    int _r, _w, _flags, _file;
    FILE *wrapped;
    char filler[120];
} bionic_FILE;

static int shim_fprintf(bionic_FILE *fp, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vfprintf(fp->wrapped, fmt, ap);
    va_end(ap);
    return r;
}
void *get_shim_fprintf(void) { return (void *)shim_fprintf; }

static int shim_fscanf(bionic_FILE *fp, const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    int r = vfscanf(fp->wrapped, fmt, ap);
    va_end(ap);
    fp->_flags = feof(fp->wrapped) ? 0x0020 : 0;
    return r;
}
void *get_shim_fscanf(void) { return (void *)shim_fscanf; }

#include <mcpelauncher/linker.h>

#include "../bionic/linker/linker_soinfo.h"
#include "../bionic/linker/linker_globals.h"
#include "../bionic/linker/linker_debug.h"
#include <cstdlib>
#include <cstdint>
#include <elf.h>

void solist_init();
soinfo* soinfo_from_handle(void* handle);

/// Register an already-loaded library (loaded by the Rust linker) with the
/// C++ bionic linker, creating a compatible soinfo so that C++ APIs like
/// HookManager::addLibrary and linker::dlsym work transparently.
/// Does NOT call link_image — relocations were already applied by the Rust
/// linker (which uses the C++ dlsym fallback for hook resolution).
/// Returns a C++ soinfo handle (void*), or 0 on failure.
extern "C" void* mcpelauncher_linker_register_loaded_library(
    const char* name,
    size_t base,
    size_t rust_handle
) {
    if (!name || !base) return nullptr;

    // Create a soinfo (adds to global solist + generates a handle)
    soinfo* si = soinfo::load_empty_library(name);
    if (!si) return nullptr;

    // Read program headers from the loaded ELF in memory.
    ElfW(Ehdr)* ehdr = reinterpret_cast<ElfW(Ehdr)*>(base);
    ElfW(Phdr)* phdr_table = reinterpret_cast<ElfW(Phdr)*>(base + ehdr->e_phoff);
    size_t phnum = ehdr->e_phnum;

    // Compute load_bias = base - min(p_vaddr of PT_LOAD).
    ElfW(Addr) min_vaddr = ~0ULL;
    for (size_t i = 0; i < phnum; i++) {
        if (phdr_table[i].p_type == PT_LOAD && phdr_table[i].p_vaddr < min_vaddr) {
            min_vaddr = phdr_table[i].p_vaddr;
        }
    }
    ElfW(Addr) computed_load_bias = (min_vaddr == ~0ULL) ? 0 : (base - min_vaddr);

    // Set base fields so prelink_image can read phdr + compute dynamic.
    // Keep size=0 so soinfo_free (if called) won't munmap — the Rust linker owns the mapping.
    si->base = base;
    si->load_bias = computed_load_bias;
    si->size = 0;
    si->phdr = phdr_table;
    si->phnum = phnum;
    si->flags_ = FLAG_NEW_SOINFO;
    si->constructors_called = 0;

    // prelink_image reads dynamic from PT_DYNAMIC (via phdr+load_bias),
    // fills strtab/symtab/hash-tables from the raw dynamic section.
    if (!si->prelink_image()) {
        fprintf(stderr, "prelink_image FAILED for %s (base=%zx, load_bias=%zx)\n",
                name, si->base, si->load_bias);
        si->base = 0;
        return nullptr;
    }
    // Mark as fully linked — relocations were applied by the Rust linker.
    si->flags_ |= FLAG_PRELINKED | FLAG_LINKED | FLAG_IMAGE_LINKED;
    si->constructors_called = 1;

    return si->to_handle();
}

namespace linker::libdl {
    std::unordered_map<std::string, void *> get_dl_symbols();
}

void linker::init() {
    const char * verbosity = getenv("MCPELAUNCHER_LINKER_VERBOSITY");
    if(verbosity) {
        g_ld_debug_verbosity = std::stoi(std::string(verbosity));
    }
    solist_init();
    linker::load_library("libdl.so", linker::libdl::get_dl_symbols());
}

extern "C" void mcpelauncher_linker_cpp_init() {
    linker::init();
}

void *linker::load_library(const char *name, const std::unordered_map<std::string, void *> &symbols) {
    auto lib = soinfo::load_library(name, symbols);
    lib->increment_ref_count();
    return lib->to_handle();
}

int linker::unload_library(void* handle) {
    auto lib = soinfo_from_handle(handle);
    if(!lib || lib->get_ref_count() != 1) {
        return 1;
    }
    
    return dlclose(handle);
}

size_t linker::get_library_base(void *handle) {
    return soinfo_from_handle(handle)->base;
}

void linker::get_library_code_region(void *handle, size_t &base, size_t &size) {
    auto s = soinfo_from_handle(handle);
    for (auto i = 0; i < s->phnum; i++) {
        if (s->phdr[i].p_type == PT_LOAD && s->phdr[i].p_flags & PF_X) {
            base = s->base + s->phdr[i].p_vaddr;
            size = s->phdr[i].p_memsz;
        }
    }
}

void linker::relocate(void *handle, const std::unordered_map<std::string, void *> &symbols) {
    auto soinfo = soinfo_from_handle(handle);
    soinfo->add_symbols(symbols);
}

extern "C" void __loader_assert(const char* file, int line, const char* msg) {
    fprintf(stderr, "linker assert failed at %s:%i: %s\n", file, line, msg);
    abort();
}

extern int do_dlclose(void* handle);

int linker::dlclose_unlocked(void* handle) {
    return do_dlclose(handle);
}
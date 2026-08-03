#include <mcpelauncher/hook.h>

#include <cstring>
#include <dlfcn.h>
#include <stdexcept>
#include <algorithm>
#include <elf.h>
#include <sys/mman.h>
#include <log.h>

// ELF macros formerly provided by the bionic linker headers
// (mcpelauncher-linker/bionic/...), kept out of the build in Phase 6.
// ElfW comes from <mcpelauncher/linker.h>'s link.h include.
#if defined(__LP64__)
#define ELFW(what) ELF64_ ## what
#else
#define ELFW(what) ELF32_ ## what
#endif

#define R_GENERIC_NONE 0 // R_*_NONE is always 0

#if defined(__x86_64__)
#define R_GENERIC_JUMP_SLOT     R_X86_64_JUMP_SLOT
#define R_GENERIC_ABSOLUTE      R_X86_64_64
#define R_GENERIC_GLOB_DAT      R_X86_64_GLOB_DAT
#elif defined(__i386__)
#define R_GENERIC_JUMP_SLOT     R_386_JMP_SLOT
#define R_GENERIC_ABSOLUTE      R_386_32
#define R_GENERIC_GLOB_DAT      R_386_GLOB_DAT
#elif defined(__aarch64__)
#define R_GENERIC_JUMP_SLOT     R_AARCH64_JUMP_SLOT
#define R_GENERIC_ABSOLUTE      R_AARCH64_ABS64
#define R_GENERIC_GLOB_DAT      R_AARCH64_GLOB_DAT
#endif

#define TAG "HookManager"

// Handle-type-agnostic dispatch wrappers (defined in mcpelauncher-linker)
extern "C" void* mcpelauncher_dispatch_dlopen(const char* name, int flags);
extern "C" int mcpelauncher_dispatch_dlclose(void* handle);

// Rust linker FFI function declarations
extern "C" size_t mcpelauncher_linker_get_rust_handle(void* cpp_handle);
extern "C" size_t mcpelauncher_linker_resolve_rust_handle(void* handle);
extern "C" size_t linker_rust_get_library_base(size_t handle);
extern "C" size_t linker_rust_get_library_dynamic(size_t handle);
extern "C" u_int32_t linker_rust_find_symbol_index_by_name(size_t handle, const char* name);

HookManager HookManager::instance;

HookManager::LibInfo::LibInfo(void *handle) : handle(handle) {
    size_t rust_handle = mcpelauncher_linker_resolve_rust_handle(handle);
    this->base = (void*) linker_rust_get_library_base(rust_handle);
    ElfW(Dyn)* dynData = (ElfW(Dyn)*) linker_rust_get_library_dynamic(rust_handle);

    for (int i = 0; ; i++) {
        if (dynData[i].d_tag == DT_NULL)
            break;
        switch (dynData[i].d_tag) {
            case DT_STRTAB:
                strtab = (const char*) ((size_t) base + dynData[i].d_un.d_ptr);
                break;
            case DT_SYMTAB:
                symtab = (ElfW(Sym)*) ((size_t) base + dynData[i].d_un.d_ptr);
                break;
            case DT_REL:
                rel = (ElfW(Rel)*) ((size_t) base + dynData[i].d_un.d_ptr);
                break;
            case DT_RELSZ:
                relsz = (ElfW(Word)) (dynData[i].d_un.d_val);
                break;
            case DT_JMPREL:
                pltrel = (ElfW(Rel)*) ((size_t) base + dynData[i].d_un.d_ptr);
                break;
            case DT_PLTRELSZ:
                pltrelsz = (ElfW(Word)) (dynData[i].d_un.d_val);
                break;
            default:
                break;
        }
    }

    ElfW(Ehdr) *header = (ElfW(Ehdr)*) base;
    for (int i = 0; i < header->e_phnum; i++) {
        ElfW(Phdr) &entry = *((ElfW(Phdr) *)
                ((size_t) base + header->e_phoff + header->e_phentsize * i));
        if (entry.p_type == PT_GNU_RELRO) {
            relro = (void*) ((size_t) base + entry.p_vaddr);
            relrosize = entry.p_memsz;
        }
    }
}

const char* HookManager::LibInfo::getSymbolName(ElfW(Word) symbolIndex) {
    return &strtab[symtab[symbolIndex].st_name];
}

void HookManager::LibInfo::setHook(
        const char *symbol_name, std::shared_ptr<HookManager::HookedSymbol> hook) {
    setHook(getSymbolIndex(handle, symbol_name), hook);
}

void HookManager::LibInfo::setHook(
        ElfW(Word) symbolIndex, std::shared_ptr<HookManager::HookedSymbol> hook) {
    hookedSymbols[symbolIndex] = hook;
}

#if defined(USE_RELA)
typedef ElfW(Rela) mcpelauncher_elf_rel;
#else
typedef ElfW(Rel) mcpelauncher_elf_rel;
#endif

void HookManager::LibInfo::applyHooks(ElfW(Rel)* rel, ElfW(Word) relsz) {
    for (size_t i = 0; i < relsz / sizeof(mcpelauncher_elf_rel); i++) {
        ElfW(Word) type = ELFW(R_TYPE)(((mcpelauncher_elf_rel*)rel)[i].r_info);
        ElfW(Word) sym = ELFW(R_SYM)(((mcpelauncher_elf_rel*)rel)[i].r_info);
        ElfW(Word)* addr = (ElfW(Word)*) ((size_t) base + ((mcpelauncher_elf_rel*)rel)[i].r_offset);
        auto found_symbol = hookedSymbols.find(sym);
        if (found_symbol == hookedSymbols.end())
            continue;
        HookedSymbol& symInfo = *found_symbol->second;
        size_t replacement = (size_t) symInfo.original;
        size_t original = 0;

        if (symInfo.lastHook != nullptr && symInfo.lastHook->replacement != nullptr)
            replacement = (size_t) symInfo.lastHook->replacement;
        else if (replacement == 0)
            continue;
        Log::trace(TAG, "Found hook for %s at %px", &strtab[symtab[sym].st_name], addr);

        switch (type) {
            case R_GENERIC_ABSOLUTE:
            case R_GENERIC_JUMP_SLOT:
            case R_GENERIC_GLOB_DAT:
                original = (size_t) *addr;
                (size_t&) *addr = replacement;
                break;
            default:
                Log::error(TAG, "Unknown relocation type: %x", type);
        }

        if (original && symInfo.original == nullptr) {
            symInfo.original = (void *) original;
            if (symInfo.firstHook)
                *symInfo.firstHook->orig = (void *) original;
        }
    }
}

void HookManager::LibInfo::applyHooks() {
    applyHooks(rel, relsz);
    applyHooks(pltrel, pltrelsz);
}

void HookManager::addLibrary(void *handle) {
    if (libs.count(handle) > 0)
        return;
    auto& p = libs[handle] = std::unique_ptr<LibInfo>(new LibInfo(handle));

    // Parse dynamic section from ELF headers in memory (handle-agnostic)
    ElfW(Ehdr)* ehdr = (ElfW(Ehdr)*) p->base;
    ElfW(Phdr)* phdr = (ElfW(Phdr)*)((size_t) p->base + ehdr->e_phoff);
    ElfW(Dyn)* dynData = nullptr;
    for (int i = 0; i < ehdr->e_phnum; i++) {
        if (phdr[i].p_type == PT_DYNAMIC) {
            dynData = (ElfW(Dyn)*)((size_t) p->base + phdr[i].p_vaddr);
            break;
        }
    }
    if (!dynData) return;

    for (int i = 0; ; i++) {
        if (dynData[i].d_tag == DT_NULL)
            break;
        if (dynData[i].d_tag == DT_NEEDED) {
            void* dep = mcpelauncher_dispatch_dlopen(&p->strtab[dynData[i].d_un.d_val], RTLD_NOLOAD);
            if (dep == nullptr)
                continue;
            p->dependencies.push_back(dep);
            dependents[dep].push_back(p.get());
            // Rust handles returned by dispatch_dlopen's name lookup do NOT hold a
            // reference (unlike __loader_dlopen), so only release C++-acquired refs.
            if (mcpelauncher_linker_resolve_rust_handle(dep) == 0) {
                mcpelauncher_dispatch_dlclose(dep);
            }
        }
    }
}

void HookManager::removeLibrary(void *handle) {
    auto p = libs.find(handle);
    if (p == libs.end())
        return;
    for (auto const& dep : p->second->dependencies)
        dependents[dep].erase(std::remove(dependents[dep].begin(), dependents[dep].end(),
                                          p->second.get()), dependents[dep].end());
    libs.erase(p);
}

HookManager::HookedSymbol* HookManager::getOrCreateHookSymbol(
        void *lib, ElfW(Word) symbolIndex) {
    auto lib_ir = libs.find(lib);
    if (lib_ir == libs.end())
        throw std::runtime_error("No such lib registered");
    LibInfo* lib_i = lib_ir->second.get();
    auto& hook = hookedSymbols[{lib_i, symbolIndex}];
    if (hook != nullptr)
        return hook.get();
    hook = std::shared_ptr<HookedSymbol>(new HookedSymbol);
    hook->lib = lib_i;
    hook->symbolIndex = symbolIndex;
    lib_i->setHook(symbolIndex, hook);
    auto name = lib_i->getSymbolName(symbolIndex);
    auto deps = dependents.find(lib);
    if (deps != dependents.end()) {
        for (LibInfo *dep : deps->second)
            dep->setHook(name, hook);
    }
    return hook.get();
}

HookManager::HookInstance* HookManager::createHook(void *lib, ElfW(Word) symbolIndex, void *replacement, void **orig) {
    auto symbol = getOrCreateHookSymbol(lib, symbolIndex);
    HookInstance* ret = new HookInstance;
    ret->symbol = symbol;
    ret->replacement = replacement;
    ret->orig = orig;
    if (ret->symbol->firstHook == nullptr) {
        ret->symbol->firstHook = ret;
    } else if (ret->symbol->lastHook != nullptr) {
        ret->parent = ret->symbol->lastHook;
        ret->parent->child = ret;
        *orig = ret->parent->replacement;
    }
    ret->symbol->lastHook = ret;
    return ret;
}

HookManager::HookInstance* HookManager::createHook(void *lib, const char *symbol_name, void *replacement, void **orig) {
    auto lib_ir = libs.find(lib);
    if (lib_ir == libs.end())
        throw std::runtime_error("No such lib registered");
    ElfW(Word) sym_index = getSymbolIndex(lib, symbol_name);
    if (sym_index == (ElfW(Word)) -1)
        throw std::runtime_error("No such symbol");
    return createHook(lib, sym_index, replacement, orig);
}

void HookManager::deleteHook(HookInstance *hook) {
    if (hook->child) {
        hook->child->parent = hook->parent;
        if (hook->parent)
            *hook->child->orig = hook->parent->replacement;
        else
            *hook->child->orig = hook->symbol->original;
    }
    if (hook->parent)
        hook->parent->child = hook->child;
    if (hook->symbol->firstHook == hook)
        hook->symbol->firstHook = hook->child;
    if (hook->symbol->lastHook == hook)
        hook->symbol->lastHook = hook->parent;
    delete hook;
}

void HookManager::applyHooks() {
    for (auto const& lib : libs)
        lib.second->applyHooks();
}

ElfW(Word) HookManager::getSymbolIndex(void *lib, const char *symbolName) {
    size_t rust_handle = mcpelauncher_linker_resolve_rust_handle(lib);
    return linker_rust_find_symbol_index_by_name(rust_handle, symbolName);
}

std::string HookManager::translateConstructorName(const char *name) {
    const char* s = name;
    auto sl = strlen(name);
    const char* se = name + sl;
    if (sl < 2 || memcmp(name, "_Z", 2) != 0)
        return std::string();
    s += 2;
    while (s < se) {
        if (*s == 'N') {
            ++s;
            continue;
        }
        char* ret;
        long len = std::strtol(s, &ret, 10);
        if (s == ret)
            break;
        s = ret + len;
    }
    if (s >= se)
        return std::string();
    if (se - s >= 2 && s[0] == 'C' && s[1] == '2') {
        std::string ret (name);
        ret[s - name + 1] = '1';
        return ret;
    }
    return std::string();
}
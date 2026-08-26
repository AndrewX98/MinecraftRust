# Catch SIGSEGV/SIGBUS/SIGILL/SIGFPE and produce a symbolized dump.
# The Rust linker mmaps game libs anonymously, so gdb knows nothing about them;
# we recover name->base from the RUST_LOG=linker=info output and attach the on-disk .so files.
# Usage: gdb -q -x tools/catch_crash.gdb --args ./target/debug/client -dg <gamedir>
set pagination off
set confirm off
handle SIGSEGV stop nopass print
handle SIGBUS   stop nopass print
handle SIGILL   stop nopass print
handle SIGFPE   stop nopass print

echo \n=== RUN (crash will be caught here) ===\n
run

echo \n=== ATTACH SYMBOLS FOR LINKER-LOADED LIBS ===\n
python
import re, glob, os

log = os.environ.get("MC_CRASH_LOG", "/tmp/mc-run.log")
pat = re.compile(r"loaded ELF '([^']+)' at ([0-9a-f]+) size (\d+)")
bases = {}
try:
    for line in open(log, errors="replace"):
        m = pat.search(line)
        if m:
            bases[m.group(1)] = int(m.group(2), 16)
except OSError as e:
    print(f"symbol-map log {log} not readable: {e}")

search_dirs = [d for d in [
    os.environ.get("MC_CRASH_GAMEDIR", ""),
    os.path.expanduser("~/.local/MinecraftLauncher/extracted/*/lib/x86_64"),
] if d]

attached = 0
for name, base in sorted(bases.items(), key=lambda kv: kv[1]):
    path = None
    for d in search_dirs:
        cand = os.path.join(d, name)
        if os.path.exists(cand):
            path = cand
            break
        hits = sorted(glob.glob(cand))
        if hits:
            path = hits[-1]
            break
    if path:
        try:
            gdb.execute(f"add-symbol-file {path} -o {base:#x}", to_string=True)
            print(f"  + {name} @ {base:#x} <- {path}")
            attached += 1
        except gdb.error as e:
            print(f"  ! {name}: {e}")
    else:
        print(f"  ? {name} @ {base:#x}: .so not found on disk")
print(f"attached symbols for {attached}/{len(bases)} libs")
end

# Every step wrapped: one failure must not abort the dump.
python
def safe(label, cmd):
    print(f"\n=== {label} ===")
    try:
        out = gdb.execute(cmd, to_string=True)
        print(out)
    except Exception as e:
        print(f"<{label}: {e}>")

safe("FAULT CONTEXT", "info program")
safe("SIGINFO", "p $_siginfo")
safe("PC DISASM (valid code => data fault; unreadable => jump-to-garbage)", "x/8i $pc")
# Hash-table crash forensics: libc++ __hash_table keeps __bucket_list_ at +0,
# so rdx (after 'mov (%rdx),%rdx') is the bucket array and rdi/r14 often the map.
safe("BUCKET ARRAY @rdx", "x/48gx $rdx")
safe("MAP/OBJECT @rdi", "x/8gx $rdi")
safe("OBJECT @r15", "x/8gx $r15")
safe("STACK TOP", "x/24gx $sp")
safe("REGISTERS", "info registers")
safe("BACKTRACE (current thread)", "bt")
safe("BACKTRACE FULL (locals)", "bt full")
safe("ALL THREADS", "thread apply all bt")
safe("MAPS", "info proc mappings")
end

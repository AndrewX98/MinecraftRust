#!/usr/bin/env python3
"""Map crash PCs from a gdb dump to (library, offset, nearest dynamic symbol).

The Rust linker maps game libs anonymously, so /proc/<pid>/maps shows no file
names. Instead it logs one line per library at startup (linker/src/lib.rs:524):

    linker: loaded ELF '<soname>' at <base-hex> size <bytes>

Capture that with:  RUST_LOG=linker=info ./target/debug/client -dg ... 2>&1 | tee run.log

Then either:
    tools/resolve_pc.py --log run.log 0x00007fff8f8b7bb1 0x00007fffacf24370
    tools/resolve_pc.py --log run.log --gdb-dump crash.txt
"""
import argparse, re, subprocess, sys, os

LOADED_RE = re.compile(r"loaded ELF '([^']+)' at ([0-9a-f]+) size (\d+)")

def parse_log(path):
    libs = []
    for line in open(path, errors="replace"):
        m = LOADED_RE.search(line)
        if m:
            libs.append((m.group(1), int(m.group(2), 16), int(m.group(3))))
    return libs

def find_lib(libs, pc):
    for name, base, size in sorted(libs, key=lambda t: -t[1]):
        if base <= pc < base + size:
            return name, base, size
    return None

def nearest_symbol(lib_path, off):
    """Nearest defined FUNC at or before `off` in .dynsym."""
    out = subprocess.run(["readelf", "-W", "--dyn-syms", lib_path],
                         capture_output=True, text=True).stdout
    best = None
    for line in out.splitlines():
        p = line.split()
        # value ndx type bind vis name -> cols: N idx VALUE SIZE TYPE BIND VIS NAME
        if len(p) >= 8 and p[6] != "UND" and p[3] in ("FUNC", "IFUNC"):
            try:
                val = int(p[1], 16)
                size = int(p[2], 0)
            except ValueError:
                continue
            if val <= off:
                cand = (val, size, p[7].split("@")[0],
                        "EXACT" if off - val < max(size, 1) else "nearest")
                if best is None or val > best[0]:
                    best = cand
    return best

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--log", required=True, help="run log containing 'loaded ELF' lines")
    ap.add_argument("--gamedir", default="/home/andrew/.local/MinecraftLauncher/extracted/26.33/lib/x86_64",
                    help="dir with the .so files, for symbol lookup")
    ap.add_argument("--gdb-dump", help="extract 'pc=...' lines / bare addresses from gdb output")
    ap.add_argument("addrs", nargs="*", help="crash addresses in hex (0x...)")
    args = ap.parse_args()

    libs = parse_log(args.log)
    if not libs:
        sys.exit(f"no 'loaded ELF' lines found in {args.log} (need RUST_LOG=linker=info)")

    addrs = [int(a, 16) for a in args.addrs]
    if args.gdb_dump:
        for line in open(args.gdb_dump):
            m = re.search(r"pc=(0x[0-9a-fA-F]+)", line)
            if m:
                addrs.append(int(m.group(1), 16))
            m = re.search(r"^#0\s+(0x[0-9a-fA-F]+)", line)
            if m:
                addrs.append(int(m.group(1), 16))

    print(f"{len(libs)} libraries loaded:")
    for name, base, size in sorted(libs, key=lambda t: t[1]):
        print(f"  {base:#012x}-{base+size:#012x}  {name}")
    print()

    for pc in addrs:
        hit = find_lib(libs, pc)
        if not hit:
            print(f"PC {pc:#x}: NOT inside any linker-loaded library "
                  f"(stack? heap? host lib?)")
            continue
        name, base, _ = hit
        off = pc - base
        so = os.path.join(args.gamedir, name)
        sym = nearest_symbol(so, off) if os.path.exists(so) else None
        loc = f"{name}+{off:#x}"
        if sym:
            val, size, sname, kind = sym
            extra = f"  -> {sname}{'' if kind=='EXACT' else f' (+{off-val:#x})'} [{kind}]"
        else:
            extra = "  -> no dynsym match"
        print(f"PC {pc:#x}: {loc}{extra}")

if __name__ == "__main__":
    main()

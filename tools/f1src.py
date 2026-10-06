"""Bridge from this workspace to the F1 workspace (read-only).

  python tools/f1src.py sources                 list registry
  python tools/f1src.py games EXT [--limit N]   list game files by extension
  python tools/f1src.py mas-list FILE           list MAS entries
  python tools/f1src.py mas-extract FILE NAME   extract one entry to %TEMP%\f1c_openengine
  python tools/f1src.py knowledge TEXT --spec-writer   query SDK findings (Team A only)

Implementers (Team B) may use every command except `knowledge`.
"""
import argparse, os, struct, subprocess, sys, tempfile, tomllib, zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REG = tomllib.loads((ROOT / "sources.toml").read_text(encoding="utf-8"))
SRC = {s["id"]: s for s in REG["source"]}
OUT = Path(tempfile.gettempdir()) / "f1c_openengine"
MAGIC = b"CUBEMAS4.10\0\0\0\0\0"


def mas_entries(path):
    b = Path(path).read_bytes()
    if b[:16] != MAGIC:
        sys.exit(f"not a CUBEMAS4.10 archive: {path}")
    n = struct.unpack_from("<I", b, 16)[0]
    base = 24 + n * 256
    for i in range(n):
        o = 24 + i * 256
        typ, off, usize, csize, _ = struct.unpack_from("<5I", b, o)
        name = b[o + 20:o + 256].split(b"\0")[0].decode("latin-1")
        yield name, typ, usize, csize, b, base + off


def unpack(b, start, usize, csize):
    raw = b[start:start + csize]
    return raw if csize == usize else zlib.decompress(raw)


def main():
    p = argparse.ArgumentParser()
    sp = p.add_subparsers(dest="cmd", required=True)
    sp.add_parser("sources")
    g = sp.add_parser("games"); g.add_argument("ext"); g.add_argument("--limit", type=int, default=50)
    g.add_argument("--source", default="game")
    m = sp.add_parser("mas-list"); m.add_argument("file")
    x = sp.add_parser("mas-extract"); x.add_argument("file"); x.add_argument("name")
    k = sp.add_parser("knowledge"); k.add_argument("text"); k.add_argument("--spec-writer", action="store_true")
    a = p.parse_args()

    if a.cmd == "sources":
        for s in REG["source"]:
            print(f"{s['tier']:9} {s['id']:20} {s['path']}")
    elif a.cmd == "games":
        root = Path(SRC[a.source]["path"]); ext = "." + a.ext.lower().lstrip(".")
        hits = [f for f in root.rglob("*") if f.suffix.lower() == ext]
        for f in hits[:a.limit]:
            print(f)
        print(f"# {len(hits)} total", file=sys.stderr)
    elif a.cmd == "mas-list":
        for name, typ, us, cs, *_ in mas_entries(a.file):
            print(f"{typ:#04x} {us:10} {cs:10} {name}")
    elif a.cmd == "mas-extract":
        for name, typ, us, cs, b, st in mas_entries(a.file):
            if name.lower() == a.name.lower():
                OUT.mkdir(exist_ok=True)
                dst = OUT / name
                dst.write_bytes(unpack(b, st, us, cs))
                print(dst); return
        sys.exit("entry not found")
    elif a.cmd == "knowledge":
        if not a.spec_writer:
            sys.exit("refused: decompile-derived knowledge is for the spec writer only (CLEAN_ROOM.md)")
        sdk = Path(SRC["sdk"]["path"])
        subprocess.run([sys.executable, "tools/f1c_mod.py", "evidence-query", "findings", a.text], cwd=sdk)


if __name__ == "__main__":
    main()

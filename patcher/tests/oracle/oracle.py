import argparse
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path


def uncomment(text):
    result = []
    i = 0
    quoted = False
    while i < len(text):
        char = text[i]
        if quoted and char == "\\":
            result.append(text[i:i + 2])
            i += 2
        elif char == '"':
            quoted = not quoted
            result.append(char)
            i += 1
        elif not quoted and text[i:i + 2] == "//":
            end = text.find("\n", i + 2)
            i = len(text) if end == -1 else end
            result.append(" ")
        elif not quoted and text[i:i + 2] == "/*":
            end = text.find("*/", i + 2)
            if end == -1:
                raise ValueError("unterminated JSON comment")
            i = end + 2
            result.append(" ")
        else:
            result.append(char)
            i += 1
    return "".join(result)


def digest(path):
    h = hashlib.sha256()
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            h.update(chunk)
    return h.hexdigest()


def first_difference(left, right):
    offset = 0
    with left.open("rb") as a, right.open("rb") as b:
        while True:
            x, y = a.read(1024 * 1024), b.read(1024 * 1024)
            if x != y:
                return offset + next((i for i, (u, v) in enumerate(zip(x, y)) if u != v), min(len(x), len(y)))
            if not x:
                return None
            offset += len(x)


def compare_states(left, right):
    a = {p.relative_to(left) for p in left.rglob("*") if p.is_file()}
    b = {p.relative_to(right) for p in right.rglob("*") if p.is_file()}
    if not a or not b:
        return "state directory is empty or missing"
    if a != b:
        return f"state files differ: C# {sorted(map(str, a))}; Rust {sorted(map(str, b))}"
    for name in sorted(a):
        x, y = left / name, right / name
        if name.suffix == ".json":
            if json.loads(x.read_text(encoding="utf-8-sig")) != json.loads(y.read_text(encoding="utf-8-sig")):
                return f"state JSON differs: {name}"
        else:
            offset = first_difference(x, y)
            if offset is not None:
                return f"state file {name} differs at offset {offset}"
    return None


def run(exe, command, addons, patches, state):
    args = [str(exe), command, "--addons", str(addons), "--patches", str(patches), "--state", str(state)]
    result = subprocess.run(args, capture_output=True, text=True, encoding="utf-8", errors="replace")
    if result.returncode:
        raise RuntimeError(f"{exe.name} {command} exit {result.returncode}: {result.stderr.strip()} {result.stdout.strip()}")
    return result.stdout


def prepare_patches(source, manifest, dest):
    shutil.copytree(source, dest, ignore=lambda directory, names: [
        name for name in names
        if Path(directory) == source and name.lower().endswith(".json") and name != manifest.name
    ])


def check(manifest, roots, csharp, rust, scratch):
    pbo = json.loads(uncomment(manifest.read_text(encoding="utf-8-sig")))["pbo"]
    if Path(pbo).name != pbo or pbo in ("", ".", ".."):
        raise ValueError(f"unsafe PBO name: {pbo}")
    matches = [root / pbo for root in roots if (root / pbo).is_file()]
    if not matches:
        raise FileNotFoundError(f"no pristine specimen for {pbo}")
    with tempfile.TemporaryDirectory(prefix="oracle-", dir=scratch) as tmp:
        root = Path(tmp)
        patches = root / "patches"
        prepare_patches(manifest.parent, manifest, patches)
        installs = []
        for label in ("csharp", "rust"):
            base = root / label
            addons = base / "mod" / "addons"
            addons.mkdir(parents=True)
            shutil.copyfile(matches[0], addons / pbo)
            installs.append((addons, base / "state"))
        for exe, (addons, state) in zip((csharp, rust), installs):
            run(exe, "apply", addons, patches, state)
        a, b = (addons / pbo for addons, _ in installs)
        offset = first_difference(a, b)
        if offset is not None:
            raise RuntimeError(f"PBO differs at offset {offset} (entry not determined)")
        state_error = compare_states(installs[0][1], installs[1][1])
        if state_error:
            raise RuntimeError(state_error)
        for exe, (addons, state) in zip((csharp, rust), installs):
            run(exe, "verify", addons, patches, state)
            path = addons / pbo
            before = digest(path), path.stat().st_mtime_ns
            output = run(exe, "apply", addons, patches, state)
            if "up to date" not in output.lower():
                raise RuntimeError(f"{exe.name} second apply did not report up to date: {output.strip()}")
            if before != (digest(path), path.stat().st_mtime_ns):
                raise RuntimeError(f"{exe.name} second apply changed PBO or timestamp")
        return pbo


def main():
    parser = argparse.ArgumentParser(description="Compare C# and Rust patcher outputs on pristine PBOs")
    parser.add_argument("--patches", required=True, type=Path)
    parser.add_argument("--specimens", required=True, action="append", type=Path)
    parser.add_argument("--csharp", required=True, type=Path)
    parser.add_argument("--rust", required=True, type=Path)
    parser.add_argument("--scratch", required=True, type=Path)
    parser.add_argument("--manifest", action="append", help="top-level manifest filename; may be repeated")
    args = parser.parse_args()
    args.scratch.mkdir(parents=True, exist_ok=True)
    manifests = sorted(p for d in args.patches.iterdir() if d.is_dir() for p in d.glob("*.json"))
    if args.manifest:
        wanted = set(args.manifest)
        manifests = [p for p in manifests if p.name in wanted]
        missing = wanted - {p.name for p in manifests}
        if missing:
            parser.error(f"manifest not found: {', '.join(sorted(missing))}")
    if not manifests:
        parser.error("no manifests found")
    print("| Manifest | Result |", flush=True)
    print("|---|---|", flush=True)
    failures = 0
    for manifest in manifests:
        try:
            check(manifest, args.specimens, args.csharp.resolve(), args.rust.resolve(), args.scratch.resolve())
            result = "PASS: bytes, state, verify, no-op"
        except (OSError, ValueError, KeyError, RuntimeError) as error:
            failures += 1
            result = "FAIL: " + str(error).replace("\n", " ").replace("|", "\\|")
        print(f"| {manifest.relative_to(args.patches).as_posix()} | {result} |", flush=True)
    return 1 if failures else 0


if __name__ == "__main__":
    sys.exit(main())

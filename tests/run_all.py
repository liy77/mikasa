#!/usr/bin/env python3
"""
Run the full Mikasa + Astra test suite.

Usage:
    python tests/run_all.py
    python tests/run_all.py --quick          # cargo lib tests only (no astra .mika)
    python tests/run_all.py --lang-only      # only tests/mikasa/*.mika
    python tests/run_all.py --examples      # also run examples/mikasa/*.mika

Exit code 0 only if every selected step passes.
"""

from __future__ import annotations

import argparse
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
from _pretty import banner, fail, info, ok, step, warn  # noqa: E402

SUITE = ROOT / "tests" / "mikasa"
EXAMPLES = ROOT / "examples" / "mikasa"


def run(cmd: list[str], *, cwd: Path | None = None, env: dict | None = None) -> int:
    print(f"  $ {' '.join(cmd)}")
    return subprocess.run(cmd, cwd=str(cwd or ROOT), env=env).returncode


def find_astra() -> Path | None:
    env_bin = os.environ.get("ASTRA_BIN")
    if env_bin:
        p = Path(env_bin)
        if p.is_file():
            return p
    candidates = [
        ROOT / "target" / "debug" / "astra",
        ROOT / "target" / "release" / "astra",
        Path("/tmp/copper-target/debug/astra"),
        Path("/tmp/copper-target/release/astra"),
    ]
    which = shutil.which("astra")
    if which:
        candidates.insert(0, Path(which))
    for c in candidates:
        if c.is_file() and os.access(c, os.X_OK):
            return c
    return None


def ensure_astra(build: bool) -> Path | None:
    existing = find_astra()
    if existing:
        return existing
    if not build:
        return None
    step("building astra")
    cargo = shutil.which("cargo")
    if not cargo:
        # rustup toolchain path (CI / minimal PATH)
        home = Path.home()
        for p in home.glob(".rustup/toolchains/*/bin/cargo"):
            cargo = str(p)
            break
    if not cargo:
        fail("cargo not found — cannot build astra")
        return None
    env = os.environ.copy()
    env.setdefault("CARGO_TARGET_DIR", "/tmp/copper-target")
    rc = run([cargo, "build", "-p", "astra-vm", "--bin", "astra", "-j", "2"], env=env)
    if rc != 0:
        fail("failed to build astra")
        return None
    return find_astra()


def cargo_tests() -> int:
    step("cargo test (mikasa-syntax, mikasa-parser, astra-vm, mika-import)")
    cargo = shutil.which("cargo")
    if not cargo:
        home = Path.home()
        for p in home.glob(".rustup/toolchains/*/bin/cargo"):
            cargo = str(p)
            break
    if not cargo:
        fail("cargo not found")
        return 1
    env = os.environ.copy()
    env.setdefault("CARGO_TARGET_DIR", "/tmp/copper-target")
    # Ensure PATH has rustc next to cargo
    cargo_path = Path(cargo)
    env["PATH"] = f"{cargo_path.parent}:{env.get('PATH', '')}"
    pkgs = [
        ["-p", "mikasa-syntax"],
        ["-p", "mikasa-parser"],
        ["-p", "astra-vm"],
        ["-p", "mika-import"],
    ]
    failed = 0
    for args in pkgs:
        rc = run([cargo, "test", *args, "--", "--test-threads=4"], env=env)
        if rc != 0:
            failed += 1
    return 1 if failed else 0


def lang_tests(astra: Path) -> int:
    step(f"language suite under {SUITE.relative_to(ROOT)} (via {astra})")
    files = sorted(
        p
        for p in SUITE.glob("*.mika")
        if p.name != "harness.mika"
    )
    if not files:
        fail("no tests/mikasa/*.mika found")
        return 1
    failed = 0
    for f in files:
        info(f"astra run {f.relative_to(ROOT)}")
        rc = run([str(astra), "run", str(f)])
        if rc != 0:
            fail(f"{f.name} failed (exit {rc})")
            failed += 1
        else:
            ok(f"{f.name}")
    return 1 if failed else 0


def example_tests(astra: Path) -> int:
    step("examples/mikasa (skip interactive / network-heavy)")
    skip = {
        "cstd.mika",  # prompts stdin
        "http.mika",
        "net.mika",
        "ws.mika",
    }
    files = []
    for p in sorted(EXAMPLES.rglob("*.mika")):
        if p.name in skip:
            continue
        # entrypoints only: main.mika or top-level *.mika (not fixtures)
        if p.parent.name in {"fixtures"}:
            continue
        if p.name in {"util.mika", "math.mika", "mathwasm.rs"}:
            continue
        if p.suffix != ".mika":
            continue
        # Prefer *main* / top-level scripts
        if p.parent == EXAMPLES or p.name == "main.mika" or p.name.endswith(".mika"):
            if p.parent.name in {"rust-interop", "rust-interop-wasm", "file-import", "multifile"}:
                if p.name != "main.mika" and p.name not in {"sieve.mika", "app.mika"}:
                    continue
            files.append(p)
    # de-dup while keeping order
    seen = set()
    uniq = []
    for f in files:
        if f in seen:
            continue
        seen.add(f)
        uniq.append(f)

    failed = 0
    for f in uniq:
        info(f"astra run {f.relative_to(ROOT)}")
        rc = run([str(astra), "run", str(f)], cwd=f.parent)
        if rc != 0:
            warn(f"{f.relative_to(ROOT)} failed (exit {rc}) — counted")
            failed += 1
        else:
            ok(str(f.relative_to(ROOT)))
    return 1 if failed else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--quick", action="store_true", help="cargo tests only")
    ap.add_argument("--lang-only", action="store_true", help="only tests/mikasa/*.mika")
    ap.add_argument("--examples", action="store_true", help="also run examples/mikasa")
    ap.add_argument("--no-build", action="store_true", help="do not cargo-build astra")
    args = ap.parse_args()

    banner("Mikasa", "Test Suite")
    rc = 0

    if not args.lang_only:
        if cargo_tests() != 0:
            rc = 1

    if args.quick:
        if rc == 0:
            ok("quick suite green")
        else:
            fail("quick suite had failures")
        return rc

    astra = ensure_astra(build=not args.no_build)
    if not astra:
        fail("astra binary not found (set ASTRA_BIN or install toolchain)")
        return 1

    if lang_tests(astra) != 0:
        rc = 1

    if args.examples:
        if example_tests(astra) != 0:
            rc = 1

    print()
    if rc == 0:
        ok("all selected tests passed")
    else:
        fail("some tests failed")
    return rc


if __name__ == "__main__":
    sys.exit(main())

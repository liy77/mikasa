#!/usr/bin/env python3
"""
Run the full Mikasa + Astra test suite.

Usage:
    python tests/run_all.py
    python tests/run_all.py --quick          # cargo lib tests only (no astra .mika)
    python tests/run_all.py --lang-only      # only tests/mikasa/*.mika
    python tests/run_all.py --examples       # also run examples/mikasa/*.mika
    python tests/run_all.py --no-build       # reuse existing astra (newest mtime)

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

# Prefer an isolated target dir (MCUDevDisk can corrupt incremental builds).
DEFAULT_TARGET = Path(os.environ.get("CARGO_TARGET_DIR", "/tmp/mikasa-target"))

# astra stdout/stderr that mean the test failed even if exit code is weird.
FAIL_MARKERS = (
    "runtime error",
    "function `",
    "not defined",
    "isn't a wasm export",
    "delegating to mikasa",
    "Build failed",
    "could not compile",
    "sintaxe @",
    "\nFAIL ",
    "FAIL ",
)


def run(
    cmd: list[str],
    *,
    cwd: Path | None = None,
    env: dict | None = None,
    capture: bool = False,
) -> tuple[int, str]:
    print(f"  $ {' '.join(cmd)}")
    if capture:
        p = subprocess.run(
            cmd,
            cwd=str(cwd or ROOT),
            env=env,
            text=True,
            capture_output=True,
        )
        out = (p.stdout or "") + (p.stderr or "")
        if out.strip():
            sys.stdout.write(out)
            if not out.endswith("\n"):
                sys.stdout.write("\n")
        return p.returncode, out
    return subprocess.run(cmd, cwd=str(cwd or ROOT), env=env).returncode, ""


def find_cargo() -> str | None:
    cargo = shutil.which("cargo")
    if cargo:
        return cargo
    home = Path.home()
    for p in home.glob(".rustup/toolchains/*/bin/cargo"):
        return str(p)
    return None


def cargo_env() -> dict:
    env = os.environ.copy()
    env["CARGO_TARGET_DIR"] = str(DEFAULT_TARGET)
    cargo = find_cargo()
    if cargo:
        env["PATH"] = f"{Path(cargo).parent}:{env.get('PATH', '')}"
    return env


def astra_candidates() -> list[Path]:
    """Newest usable astra first — never silently prefer a stale tree binary."""
    env_bin = os.environ.get("ASTRA_BIN")
    out: list[Path] = []
    if env_bin:
        out.append(Path(env_bin))
    out.extend(
        [
            DEFAULT_TARGET / "debug" / "astra",
            DEFAULT_TARGET / "release" / "astra",
            ROOT / "target" / "debug" / "astra",
            ROOT / "target" / "release" / "astra",
        ]
    )
    which = shutil.which("astra")
    if which:
        out.append(Path(which))
    # unique, existing, executable, sorted by mtime desc
    seen: set[Path] = set()
    existing: list[Path] = []
    for c in out:
        try:
            c = c.resolve()
        except OSError:
            continue
        if c in seen or not c.is_file() or not os.access(c, os.X_OK):
            continue
        seen.add(c)
        existing.append(c)
    existing.sort(key=lambda p: p.stat().st_mtime, reverse=True)
    return existing


def ensure_astra(*, rebuild: bool) -> Path | None:
    env = cargo_env()
    built = DEFAULT_TARGET / "debug" / "astra"
    if rebuild:
        cargo = find_cargo()
        if not cargo:
            fail("cargo not found — cannot build astra")
            return None
        step(f"building fresh astra → {built}")
        rc, _ = run(
            [cargo, "build", "-p", "astra-vm", "--bin", "astra", "-j", "2"],
            env=env,
        )
        if rc != 0:
            fail("failed to build astra")
            return None
        if built.is_file():
            info(f"using {built}")
            return built

    cands = astra_candidates()
    if not cands:
        return None
    chosen = cands[0]
    info(f"using astra {chosen} (mtime newest)")
    if len(cands) > 1:
        older = cands[1]
        if older.stat().st_mtime < chosen.stat().st_mtime:
            warn(
                f"ignored older binary {older} — "
                f"rebuild with default (omit --no-build) if tests look stale"
            )
    return chosen


def cargo_tests() -> int:
    step("cargo test (mikasa-syntax, mikasa-parser, astra-vm, mika-import)")
    cargo = find_cargo()
    if not cargo:
        fail("cargo not found")
        return 1
    env = cargo_env()
    pkgs = [
        ["-p", "mikasa-syntax"],
        ["-p", "mikasa-parser"],
        ["-p", "astra-vm"],
        ["-p", "mika-import"],
    ]
    failed = 0
    for args in pkgs:
        rc, _ = run([cargo, "test", *args, "--", "--test-threads=4"], env=env)
        if rc != 0:
            failed += 1
    return 1 if failed else 0


def output_indicates_failure(text: str) -> str | None:
    lower = text.lower()
    for marker in FAIL_MARKERS:
        if marker.lower() in lower:
            return marker
    # harness summary
    if "failed --" in text and not text.strip().endswith("0 failed --"):
        # e.g. "-- 3 passed, 2 failed --"
        for line in text.splitlines():
            if "passed" in line and "failed" in line and not line.strip().startswith("-- 0"):
                if ", 0 failed" in line or "0 failed --" in line:
                    continue
                if "failed --" in line:
                    return line.strip()
    return None


def lang_tests(astra: Path) -> int:
    step(f"language suite under {SUITE.relative_to(ROOT)} (via {astra})")
    files = sorted(
        p for p in SUITE.glob("*.mika") if p.name != "harness.mika"
    )
    if not files:
        fail("no tests/mikasa/*.mika found")
        return 1
    failed = 0
    for f in files:
        info(f"astra run {f.relative_to(ROOT)}")
        rc, out = run([str(astra), "run", str(f)], capture=True)
        marker = output_indicates_failure(out)
        if rc != 0 or marker:
            why = f"exit {rc}" if rc != 0 else f"marker {marker!r}"
            fail(f"{f.name} failed ({why})")
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
    files: list[Path] = []
    for p in sorted(EXAMPLES.rglob("*.mika")):
        if p.name in skip:
            continue
        if p.parent.name == "fixtures":
            continue
        if p.name in {"util.mika", "math.mika"}:
            continue
        if p.parent.name in {"rust-interop", "rust-interop-wasm", "file-import", "multifile"}:
            if p.name not in {"main.mika", "sieve.mika", "app.mika"}:
                continue
        files.append(p)

    failed = 0
    for f in files:
        info(f"astra run {f.relative_to(ROOT)}")
        rc, out = run([str(astra), "run", str(f)], cwd=f.parent, capture=True)
        marker = output_indicates_failure(out)
        if rc != 0 or marker:
            warn(f"{f.relative_to(ROOT)} failed — counted")
            failed += 1
        else:
            ok(str(f.relative_to(ROOT)))
    return 1 if failed else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--quick", action="store_true", help="cargo tests only")
    ap.add_argument("--lang-only", action="store_true", help="only tests/mikasa/*.mika")
    ap.add_argument("--examples", action="store_true", help="also run examples/mikasa")
    ap.add_argument(
        "--no-build",
        action="store_true",
        help="do not rebuild astra (use newest existing binary)",
    )
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

    # Always rebuild by default so we never run language tests on a stale
    # target/debug/astra that predates import("…") / wasm wrap.
    astra = ensure_astra(rebuild=not args.no_build)
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

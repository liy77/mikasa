#!/usr/bin/env python3
"""
Print install diagnostics for mikasa: where it lives, whether it's on PATH,
the MIKASA_PATH variable, and whether the binary actually runs.

Cross-platform replacement for the old diagnose.bat (Windows-only).

Usage:
    python scripts/diagnose.py
    python scripts/diagnose.py --local | --global   # inspect a specific scope
"""

import argparse
import os
import platform
import shutil
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from _pretty import banner, fail, head, info, ok, warn  # noqa: E402

SYS = platform.system()


def is_admin():
    if SYS == "Windows":
        try:
            import ctypes
            return ctypes.windll.shell32.IsUserAnAdmin() != 0
        except Exception:
            return False
    return os.geteuid() == 0


def resolve_scope(force):
    glob = (force == "global") or (force is None and is_admin())
    if SYS == "Windows":
        install_dir = Path(r"C:\Program Files\Mikasa") if glob else Path(os.environ["USERPROFILE"]) / ".mikasa"
    else:
        install_dir = Path("/usr/local/lib/mikasa") if glob else Path.home() / ".mikasa"
    return ("global" if glob else "local"), install_dir


def registry_value(scope, name):
    if SYS != "Windows":
        return None
    import winreg
    if scope == "global":
        root, subkey = winreg.HKEY_LOCAL_MACHINE, r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment"
    else:
        root, subkey = winreg.HKEY_CURRENT_USER, "Environment"
    try:
        with winreg.OpenKey(root, subkey) as key:
            val, _ = winreg.QueryValueEx(key, name)
            return val
    except FileNotFoundError:
        return None


def main():
    ap = argparse.ArgumentParser(prog="diagnose.py", description="Diagnose a mikasa install.")
    grp = ap.add_mutually_exclusive_group()
    grp.add_argument("--local", action="store_const", const="local", dest="force")
    grp.add_argument("--global", action="store_const", const="global", dest="force")
    args = ap.parse_args()

    banner("Mikasa", "Diagnostic Tool")
    scope, install_dir = resolve_scope(args.force)
    info(f"Privilege: {'admin/root' if is_admin() else 'normal user'}  →  inspecting {scope} scope")
    info(f"Expected install dir: {install_dir}")

    # 1. Install directory + binary.
    head("Installation")
    exe = install_dir / "bin" / ("mikasa.exe" if SYS == "Windows" else "mikasa")
    if install_dir.exists():
        ok(f"Install directory found: {install_dir}")
        if exe.exists():
            stat = exe.stat()
            ok(f"Executable found: {exe}")
            info(f"Size: {stat.st_size} bytes")
        else:
            fail(f"Executable NOT found: {exe}")
        if (install_dir / "Cargo.toml").exists():
            ok("Project metadata found (Cargo.toml)")
        else:
            warn("Project metadata missing (Cargo.toml)")
    else:
        fail(f"Install directory NOT found: {install_dir}")

    # 2. PATH / MIKASA_PATH.
    head("Environment")
    if SYS == "Windows":
        mikasa_path = registry_value(scope, "MIKASA_PATH")
        reg_path = registry_value(scope, "PATH") or ""
        if mikasa_path:
            ok(f"MIKASA_PATH = {mikasa_path}")
        else:
            warn("MIKASA_PATH not set in this scope's registry.")
        if r"%MIKASA_PATH%\bin" in reg_path or str(install_dir / "bin") in reg_path:
            ok("Mikasa bin directory present in persisted PATH.")
        else:
            fail("Mikasa bin directory NOT in persisted PATH.")
    else:
        mikasa_path = os.environ.get("MIKASA_PATH")
        ok(f"MIKASA_PATH = {mikasa_path}") if mikasa_path else warn("MIKASA_PATH not set in this shell.")
        if str(install_dir / "bin") in os.environ.get("PATH", ""):
            ok("Mikasa bin directory present in current PATH.")
        else:
            warn("Mikasa bin directory NOT in current shell PATH (open a new terminal?).")

    # 3. Does mikasa actually run?
    head("Execution")
    found = shutil.which("mikasa")
    target = found or (str(exe) if exe.exists() else None)
    if found:
        ok(f"mikasa resolved on PATH: {found}")
    else:
        warn("mikasa not on the current PATH; trying the install dir directly.")
    if target:
        try:
            res = subprocess.run([target, "--version"], capture_output=True, text=True)
            if res.returncode == 0:
                ok(f"mikasa runs: {res.stdout.strip().splitlines()[0] if res.stdout.strip() else 'ok'}")
            else:
                fail("mikasa found but exited non-zero on --version.")
        except OSError as e:
            fail(f"mikasa could not be executed: {e}")
    else:
        fail("No mikasa binary to test — run python scripts/install.py.")

    banner("Diagnostic complete")
    if exe.exists():
        info("If mikasa isn't recognised, open a NEW terminal so PATH refreshes.")
    else:
        info("Installation not found or incomplete — run python scripts/install.py.")
    print()


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Alias for `python tests/run_all.py` — same flags, same exit code."""

import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.exit(subprocess.call([sys.executable, str(ROOT / "tests" / "run_all.py"), *sys.argv[1:]]))

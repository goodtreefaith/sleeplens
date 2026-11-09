#!/usr/bin/env python3
"""Copy the current React sample-mode build to the public demo directory."""
from pathlib import Path
import shutil

HERE = Path(__file__).resolve().parent
SOURCE = HERE.parent / "app" / "dist"
assert (SOURCE / "index.html").is_file(), "Run npm --prefix app run build:demo"
shutil.copy2(SOURCE / "index.html", HERE / "index.html")
for name in ("sleeplens-owl.png", "favicon.png"):
    shutil.copy2(SOURCE / name, HERE / name)
shutil.copytree(SOURCE / "assets", HERE / "assets", dirs_exist_ok=True)
shutil.copytree(SOURCE / "mascots", HERE / "mascots", dirs_exist_ok=True)
# Remove only stale generated bundles, keeping build source and documentation.
for target in (HERE / "assets").iterdir():
    if not (SOURCE / "assets" / target.name).exists():
        target.unlink()

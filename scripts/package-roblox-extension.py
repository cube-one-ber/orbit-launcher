#!/usr/bin/env python3
"""Package source-only browser variants; Firefox still requires Mozilla signing."""
from pathlib import Path
import sys
import zipfile

source = Path(__file__).resolve().parents[1] / "browser-extension" / "roblox"
output = Path(sys.argv[1]) if len(sys.argv) > 1 else Path("target/artifacts")
output.mkdir(parents=True, exist_ok=True)
files = ("report.js", "content.js", "background.js", "popup.html", "popup.css", "popup.js")
for browser, manifest in (("chromium", "manifest.json"), ("firefox", "manifest.firefox.json")):
    archive = output / f"orbit-roblox-{browser}.zip"
    with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as package:
        package.write(source / manifest, "manifest.json")
        for name in files:
            package.write(source / name, name)
    print(archive)

"""Archive CI binaries without losing Unix executable permissions."""

from pathlib import Path
import shutil
import sys


target, platform = sys.argv[1:]
workspace = Path(__file__).resolve().parent.parent
name = f"orbit-{platform}"
stage = workspace / "target" / "ci-package" / name
stage.mkdir(parents=True, exist_ok=True)
binary = "orbit.exe" if platform.startswith("windows-") else "orbit"
shutil.copy2(workspace / "target" / target / "release" / binary, stage / binary)
for document in ("LICENSE", "README.md", "docs/ci.md"):
    shutil.copy2(workspace / document, stage / Path(document).name)
artifacts = workspace / "target" / "artifacts"
artifacts.mkdir(parents=True, exist_ok=True)
archive_format = "zip" if platform.startswith("windows-") else "gztar"
archive = shutil.make_archive(str(artifacts / name), archive_format, stage.parent, name)
print(f"Created {archive}")

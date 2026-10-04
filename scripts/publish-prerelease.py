"""Publish one validated prerelease per commit, with retry-safe uploads."""

import hashlib
import os
from pathlib import Path
import subprocess
import sys
import tomllib


EXPECTED_ASSETS = (
    "orbit-linux-x86_64.tar.gz", "orbit-windows-x86_64.zip",
    "orbit-macos-x86_64.tar.gz", "orbit-macos-arm64.tar.gz",
    "orbit-roblox-chromium.zip", "orbit-roblox-firefox.zip",
)


def prepare_assets(directory):
    assets = [directory / name for name in EXPECTED_ASSETS]
    missing = [asset.name for asset in assets if not asset.is_file() or asset.stat().st_size == 0]
    if missing:
        raise RuntimeError(f"Refusing to publish an incomplete release: {', '.join(missing)}")
    checksums = directory / "SHA256SUMS"
    lines = []
    for asset in assets:
        with asset.open("rb") as source:
            lines.append(f"{hashlib.file_digest(source, 'sha256').hexdigest()}  {asset.name}\n")
    checksums.write_text("".join(lines), encoding="utf-8")
    return [*assets, checksums]


def main():
    directory = Path(sys.argv[1]).resolve()
    assets = prepare_assets(directory)
    sha = os.environ["GITHUB_SHA"]
    repository = os.environ["GITHUB_REPOSITORY"]
    ref = os.environ["GITHUB_REF_NAME"]
    tag = f"ci-{sha}"
    version = tomllib.loads((Path(__file__).resolve().parent.parent / "Cargo.toml").read_text())["package"]["version"]
    notes = directory / "release-notes.md"
    notes.write_text(
        f"Automated prerelease of Orbit {version} from `{ref}` at `{sha}`.\n\n"
        "All Rust/browser tests, builds, UI checks and extracted-package checks passed.\n\n"
        "- **Windows 10/11 x64:** extract the entire ZIP and run `orbit.exe`. Qt, Kirigami, image plugins and the Microsoft C++ runtime are included.\n"
        "- **Linux x64 (Ubuntu 24.04 or newer):** extract the tar.gz and run `./orbit`. Qt/Kirigami and native dependencies are bundled; system graphics drivers and glibc are required.\n"
        "- **macOS Intel/Apple Silicon (macOS 15+):** extract the matching tar.gz and open `Orbit.app`. Runtime libraries are bundled. The app is ad-hoc signed and not notarized.\n"
        "- Roblox extension archives are included; Firefox's source archive is unsigned.\n\n"
        "Archives include dependency licenses and source locations. `SHA256SUMS` covers every download.\n"
        "Game clients must still be installed to play games. macOS native game discovery remains incomplete.\n",
        encoding="utf-8")
    common = ["--repo", repository]
    existing = subprocess.run(["gh", "release", "view", tag, *common], capture_output=True, text=True)
    if existing.returncode == 0:
        subprocess.run(["gh", "release", "upload", tag, *map(str, assets), "--clobber", *common], check=True)
    else:
        # Upload to a draft first; failed uploads cannot publish a partial release.
        subprocess.run(["gh", "release", "create", tag, *map(str, assets), "--target", sha,
                        "--draft", "--prerelease", "--title", f"Orbit {version} pre-release ({sha[:12]})",
                        "--notes-file", str(notes), *common], check=True)
    subprocess.run(["gh", "release", "edit", tag, "--draft=false", "--prerelease",
                    "--latest=false", "--notes-file", str(notes), *common], check=True)
    url = f"https://github.com/{repository}/releases/tag/{tag}"
    print(url)
    if "GITHUB_STEP_SUMMARY" in os.environ:
        with open(os.environ["GITHUB_STEP_SUMMARY"], "a", encoding="utf-8") as summary:
            summary.write(f"[Download this commit's prerelease]({url})\n")


if __name__ == "__main__":
    main()

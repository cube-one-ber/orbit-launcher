"""Include pinned dependency licenses, attributions, and source locations."""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import urllib.request


def main():
    workspace = Path(__file__).resolve().parent.parent
    stage = Path(sys.argv[1])
    licenses = stage / "licenses"
    licenses.mkdir()
    metadata = json.loads(subprocess.check_output(
        ["cargo", "metadata", "--locked", "--format-version", "1"], text=True, cwd=workspace))
    notices = ["# Third-party software\n", "Qt and Kirigami are dynamically linked and can be replaced with compatible builds.",
               "License texts and upstream attributions are included in licenses/.\n", "## Rust crates\n"]
    for package in sorted(metadata["packages"], key=lambda item: (item["name"], item["version"])):
        if package["source"] is None:
            continue
        name = f'{package["name"]}-{package["version"]}'
        directory = Path(package["manifest_path"]).parent
        destination = licenses / "rust" / name
        destination.mkdir(parents=True)
        for file in directory.iterdir():
            if file.name.lower().startswith(("license", "copying", "notice", "copyright")):
                if file.is_dir():
                    shutil.copytree(file, destination / file.name)
                else:
                    shutil.copy2(file, destination / file.name)
        notices.append(f'- {name}: {package["license"] or "see upstream license files"}; https://crates.io/crates/{package["name"]}/{package["version"]}')

    notices.append("\n## Qt and KDE sources\n")
    for owner, project, version in [
        *(('qt', project, os.environ['QT_VERSION']) for project in ('qtbase', 'qtdeclarative', 'qtsvg', 'qtshadertools')),
        ('KDE', 'kirigami', os.environ['KDE_VERSION']),
    ]:
        cache = workspace / "target/ci-license-sources" / f"{project}-{version}"
        if not (cache / ".complete").exists():
            cache.mkdir(parents=True, exist_ok=True)
            download = cache / "source.tar.gz"
            urllib.request.urlretrieve(f"https://codeload.github.com/{owner}/{project}/tar.gz/refs/tags/v{version}", download)
            with tarfile.open(download) as archive:
                for member in archive:
                    relative = Path(*Path(member.name).parts[1:])
                    if not member.isfile() or ".." in relative.parts:
                        continue
                    if ("LICENSES" in relative.parts or relative.name.lower().startswith(("license", "copying", "notice", "copyright"))
                            or relative.name == "qt_attributions.json"):
                        destination = cache / relative
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        with archive.extractfile(member) as source, destination.open("wb") as output:
                            shutil.copyfileobj(source, output)
            download.unlink()
            (cache / ".complete").touch()
        shutil.copytree(cache, licenses / f"{project}-{version}", ignore=shutil.ignore_patterns(".complete"))
        notices.append(f'- {project} {version}: https://github.com/{owner}/{project}/tree/v{version}; source archive: https://github.com/{owner}/{project}/archive/refs/tags/v{version}.tar.gz')
    if os.name == "nt":
        notices.append("\n## Microsoft runtime\nMicrosoft Visual C++ runtime DLLs come from Visual Studio's official redistributable directories. Redistribution terms: https://learn.microsoft.com/cpp/windows/redistributing-visual-cpp-files")
    elif sys.platform == "linux":
        for library in (stage / "lib").iterdir():
            result = subprocess.run(["dpkg-query", "-S", f"/usr/lib/x86_64-linux-gnu/{library.name}"], capture_output=True, text=True)
            for line in result.stdout.splitlines():
                package = line.split(": ", 1)[0].split(":", 1)[0]
                copyright = Path("/usr/share/doc") / package / "copyright"
                if copyright.is_file():
                    destination = licenses / "system" / package
                    destination.mkdir(parents=True, exist_ok=True)
                    shutil.copy2(copyright, destination / "copyright")
    (stage / "THIRD-PARTY-NOTICES.md").write_text("\n".join(notices) + "\n", encoding="utf-8")


if __name__ == "__main__":
    main()

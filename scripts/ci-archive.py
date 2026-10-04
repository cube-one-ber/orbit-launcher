"""Deploy Qt/Kirigami runtimes and archive complete, relocatable applications."""

import json
import os
from pathlib import Path
import plistlib
import re
import shutil
import subprocess
import sys
import tomllib


WORKSPACE = Path(__file__).resolve().parent.parent


def run(*args, **kwargs):
    return subprocess.run([str(arg) for arg in args], check=True, **kwargs)


def copy_tree(source, destination):
    shutil.copytree(source, destination, dirs_exist_ok=True, symlinks=True,
                    ignore=shutil.ignore_patterns("*.pdb", "*.debug", "*.dSYM"))


def windows_runtime(stage, qt, kde):
    copy_tree(kde / "qml", stage / "qml")
    run(qt / "bin/windeployqt.exe", "--release", "--no-compiler-runtime",
        "--qmldir", WORKSPACE / "qml", "--qmlimport", kde / "qml",
        "--dir", stage, stage / "orbit.exe")
    # Image/SVG plugins may be loaded at runtime rather than appear in PE imports.
    for plugin_type in ("imageformats", "iconengines"):
        destination = stage / plugin_type
        destination.mkdir(exist_ok=True)
        for plugin in (qt / "plugins" / plugin_type).glob("*.dll"):
            if not plugin.stem.endswith("d"):
                shutil.copy2(plugin, destination / plugin.name)
    # Only Microsoft's redistributable directories, never compiler/debug DLLs.
    redist = Path(os.environ["VCToolsRedistDir"]) / "x64"
    runtime_dirs = sorted(redist.glob("Microsoft.VC*.CRT")) + sorted(redist.glob("Microsoft.VC*.OpenMP"))
    if not runtime_dirs:
        raise RuntimeError(f"MSVC redistributable DLLs not found in {redist}")
    for directory in runtime_dirs:
        for library in directory.glob("*.dll"):
            shutil.copy2(library, stage / library.name)
    for required in ("msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll"):
        if not (stage / required).is_file():
            raise RuntimeError(f"Missing Microsoft runtime: {required}")

    # Follow every PE import, including QML plugins. windeployqt does not
    # deploy arbitrary KDE dependencies. Only OS DLLs may be left external.
    search_dirs = [qt / "bin", kde / "bin", *runtime_dirs]
    available = {library.name.lower(): library
                 for directory in search_dirs for library in directory.glob("*.dll")}
    system = Path(os.environ["SystemRoot"]) / "System32"
    queue = list(stage.rglob("*.dll")) + [stage / "orbit.exe"]
    seen = set()
    while queue:
        binary = queue.pop()
        if binary in seen:
            continue
        seen.add(binary)
        output = run("dumpbin.exe", "/DEPENDENTS", binary, capture_output=True, text=True).stdout
        for name in re.findall(r"^\s+([\w.+-]+\.dll)\s*$", output, re.MULTILINE | re.IGNORECASE):
            destination = stage / name
            source = available.get(name.lower())
            if not destination.is_file() and source:
                shutil.copy2(source, destination)
                queue.append(destination)
            elif not destination.is_file() and not name.lower().startswith(("api-ms-", "ext-ms-")) and not (system / name).is_file():
                raise RuntimeError(f"Unresolved Windows dependency: {binary}: {name}")
    (stage / "qt.conf").write_text("[Paths]\nPrefix=.\nPlugins=.\nQmlImports=qml\n", encoding="utf-8")
    (stage / "START-HERE.txt").write_text(
        "Extract this entire ZIP, then double-click orbit.exe.\n"
        "Qt, Kirigami, SVG/image plugins and the Microsoft C++ runtime are included.\n"
        "Windows 10/11 x64 is required. Keep all DLLs and the qml/plugins folders.\n"
        "Game clients and browsers must still be installed to launch their games.\n", encoding="utf-8")


def linux_runtime(stage, qt, kde):
    (stage / "lib").mkdir()
    for prefix in (qt, kde):
        for library in (prefix / "lib").glob("*.so*"):
            shutil.copy2(library, stage / "lib" / library.name, follow_symlinks=False)
    copy_tree(qt / "qml", stage / "qml")
    copy_tree(kde / "qml", stage / "qml")
    copy_tree(qt / "plugins", stage / "plugins")
    # glibc and graphics drivers remain provided by the Linux distribution.
    system_libraries = re.compile(r"^(ld-linux|lib(c|m|mvec|dl|pthread|rt|resolv|util|anl)\.so|libnss_|lib(GL|EGL|GLX|GLdispatch|OpenGL)\.)")
    queue = [stage / "bin/orbit", *stage.rglob("*.so*")]
    seen = set()
    environment = dict(os.environ, LD_LIBRARY_PATH=str(stage / "lib"))
    while queue:
        binary = queue.pop()
        identity = binary.resolve()
        if identity in seen or not binary.is_file():
            continue
        seen.add(identity)
        output = run("ldd", binary, capture_output=True, text=True, env=environment).stdout
        if "not found" in output:
            raise RuntimeError(f"Unresolved Linux dependency: {binary}\n{output}")
        for name, path in re.findall(r"^\s*(\S+) => (/\S+) \(", output, re.MULTILINE):
            destination = stage / "lib" / name
            if not system_libraries.match(name) and not destination.exists():
                shutil.copy2(path, destination)
                queue.append(destination)
    (stage / "bin/qt.conf").write_text("[Paths]\nPrefix=..\nPlugins=plugins\nQmlImports=qml\n", encoding="utf-8")
    launcher = stage / "orbit"
    launcher.write_text(
        '#!/bin/sh\nset -eu\n'
        'orbit_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)\n'
        'export LD_LIBRARY_PATH="$orbit_dir/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"\n'
        'export QT_PLUGIN_PATH="$orbit_dir/plugins"\n'
        'export QML_IMPORT_PATH="$orbit_dir/qml"\n'
        'export QML2_IMPORT_PATH="$orbit_dir/qml"\n'
        'exec "$orbit_dir/bin/orbit" "$@"\n', encoding="utf-8")
    launcher.chmod(0o755)


def macos_runtime(stage, qt, kde, version):
    app = stage / "Orbit.app"
    contents = app / "Contents"
    (contents / "Resources").mkdir(parents=True)
    with (contents / "Info.plist").open("wb") as output:
        plistlib.dump({"CFBundleExecutable": "orbit", "CFBundleIdentifier": "app.orbit.Launcher",
                      "CFBundleName": "Orbit", "CFBundlePackageType": "APPL",
                      "CFBundleShortVersionString": version, "CFBundleVersion": version,
                      "NSHighResolutionCapable": True, "LSMinimumSystemVersion": "15.0"}, output)
    copy_tree(kde / "qml", contents / "Resources/qml")
    run(qt / "bin/macdeployqt", app, f"-qmldir={WORKSPACE / 'qml'}",
        f"-qmlimport={kde / 'qml'}", f"-libpath={kde / 'lib'}",
        "-always-overwrite", "-verbose=2")
    # Ad-hoc signing supports ARM64 execution. Notarization needs Apple credentials.
    run("codesign", "--force", "--deep", "--sign", "-", app)
    run("codesign", "--verify", "--deep", "--strict", app)


def main():
    target, platform = sys.argv[1:]
    qt = Path(os.environ["QT_ROOT_DIR"]).resolve()
    kde = WORKSPACE / "target/ci-deps"
    version = tomllib.loads((WORKSPACE / "Cargo.toml").read_text())["package"]["version"]
    name = f"orbit-{platform}"
    stage = WORKSPACE / "target/ci-package" / name
    if stage.exists():
        shutil.rmtree(stage)
    stage.mkdir(parents=True)
    binary = "orbit.exe" if platform.startswith("windows-") else "orbit"
    destination = stage / ("Orbit.app/Contents/MacOS/orbit" if platform.startswith("macos-")
                           else "bin/orbit" if platform.startswith("linux-") else binary)
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(WORKSPACE / "target" / target / "release" / binary, destination)
    if platform.startswith("windows-"):
        windows_runtime(stage, qt, kde)
    elif platform.startswith("linux-"):
        linux_runtime(stage, qt, kde)
    else:
        macos_runtime(stage, qt, kde, version)
    for document in ("LICENSE", "docs/ci.md"):
        shutil.copy2(WORKSPACE / document, stage / Path(document).name)
    (stage / "BUILD-INFO.json").write_text(json.dumps({
        "version": version, "commit": os.environ["GITHUB_SHA"], "platform": platform,
        "qt": os.environ["QT_VERSION"], "kirigami": os.environ["KDE_VERSION"],
    }, indent=2) + "\n", encoding="utf-8")
    run(sys.executable, WORKSPACE / "scripts/ci-licenses.py", stage)
    artifacts = WORKSPACE / "target/artifacts"
    artifacts.mkdir(parents=True, exist_ok=True)
    archive = shutil.make_archive(str(artifacts / name), "zip" if platform.startswith("windows-") else "gztar", stage.parent, name)
    print(f"Created runtime bundle: {archive}")


if __name__ == "__main__":
    main()

"""Test the extracted release artifact with development paths removed."""

import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tarfile
import tempfile
import zipfile

from linux_runtime import host_library


def main():
    platform, archive_path = sys.argv[1:]
    archive_path = Path(archive_path).resolve()
    with tempfile.TemporaryDirectory(prefix="orbit relocated ") as temporary:
        directory = Path(temporary) / "with spaces"
        directory.mkdir()
        if archive_path.suffix == ".zip":
            with zipfile.ZipFile(archive_path) as archive:
                archive.extractall(directory)
        else:
            with tarfile.open(archive_path) as archive:
                archive.extractall(directory, filter="data")
        package = directory / f"orbit-{platform}"
        info = json.loads((package / "BUILD-INFO.json").read_text())
        if info["commit"] != os.environ["GITHUB_SHA"] or info["platform"] != platform:
            raise RuntimeError("Package does not match the tested commit/architecture")
        if not (package / "THIRD-PARTY-NOTICES.md").is_file():
            raise RuntimeError("Package is missing dependency notices")
        if platform.startswith("windows-"):
            executable = package / "orbit.exe"
            for file in ("Qt6Core.dll", "Qt6Gui.dll", "Qt6Qml.dll", "Qt6Quick.dll",
                         "msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll",
                         "platforms/qwindows.dll", "platforms/qoffscreen.dll", "imageformats/qsvg.dll",
                         "qml/org/kde/kirigami/qmldir"):
                if not (package / file).is_file():
                    raise RuntimeError(f"Windows bundle is missing {file}")
        elif platform.startswith("macos-"):
            executable = package / "Orbit.app/Contents/MacOS/orbit"
            subprocess.run(["codesign", "--verify", "--deep", "--strict", str(package / "Orbit.app")], check=True)
        else:
            executable = package / "orbit"
            conflicts = [library.name for library in (package / "lib").iterdir()
                         if host_library(library.name)]
            if conflicts:
                raise RuntimeError(f"Linux bundle overrides host graphics/font libraries: {sorted(conflicts)}")
        environment = {key: value for key, value in os.environ.items()
                       if not key.startswith(("QT_", "QML", "QSG_", "LIBGL_", "DYLD_", "LD_LIBRARY_PATH", "QMAKE"))}
        if os.name == "nt":
            import ctypes
            # Child GUI failures must return an error rather than wait forever
            # in Windows loader/crash dialogs on an unattended runner.
            ctypes.windll.kernel32.SetErrorMode(0x0001 | 0x0002 | 0x8000)
            environment["PATH"] = f"{package};{os.environ['SystemRoot']}/System32;{os.environ['SystemRoot']}"
        else:
            environment["PATH"] = "/usr/bin:/bin:/usr/sbin:/sbin"
        environment.update(QT_FORCE_STDERR_LOGGING="1", ORBIT_CONFIG_DIR=str(directory / "config"))
        for arguments, offscreen in [(["--demo", "--ui-test"], True),
                                     (["--demo", "--smoke-test"], False),
                                     (["--demo", "--light", "--smoke-test"], True)]:
            test_environment = dict(environment)
            command = [str(executable), *arguments]
            if offscreen:
                test_environment["QT_QPA_PLATFORM"] = "offscreen"
                test_environment["QT_QUICK_BACKEND"] = "software"
            elif platform.startswith("linux-"):
                # Exercise Qt's real RHI/OpenGL path using Mesa's CPU driver.
                # QT_QUICK_BACKEND=software here would hide broken EGL/GL loading.
                test_environment.update(QT_QPA_PLATFORM="xcb", LIBGL_ALWAYS_SOFTWARE="1", QSG_RHI_BACKEND="opengl", QSG_INFO="1")
                command = ["/usr/bin/xvfb-run", "-a", *command]
            else:
                test_environment["QT_QUICK_BACKEND"] = "software"
            try:
                result = subprocess.run(command, cwd=directory, env=test_environment,
                                        capture_output=True, text=True, timeout=45)
            except subprocess.TimeoutExpired as error:
                print(error.stdout or b"", error.stderr or b"", flush=True)
                raise
            log = result.stdout + result.stderr
            print(log, flush=True)
            if result.returncode != 0 or re.search(r"ORBIT_UI_TEST_FAIL|ReferenceError|TypeError|Binding loop|Cannot assign|Unable to assign|Could not load|Fontconfig warning|EGL not available|Failed to create (?:temporary )?context|Failed to initialize graphics backend", log):
                raise RuntimeError(f"Extracted package failed {arguments}: exit {result.returncode}")
            if "--ui-test" in arguments and "ORBIT_UI_TEST_PASS" not in log:
                raise RuntimeError("Extracted package UI checks did not complete")
            if platform.startswith("linux-") and not offscreen and "Creating QRhi with backend OpenGL" not in log:
                raise RuntimeError("Linux native smoke test did not exercise the OpenGL renderer")
        if (directory / "config/settings.json").exists():
            raise RuntimeError("Package preview wrote user settings")
        print(f"PACKAGE_TEST_PASS: {platform}; relocated artifact, isolated paths, UI, native graphics and light smoke checks")


if __name__ == "__main__":
    main()

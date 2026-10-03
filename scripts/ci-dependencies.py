"""Build a matching Kirigami runtime and export paths for later CI steps."""

import os
from pathlib import Path
import subprocess


def run(*args):
    subprocess.run(args, check=True)


workspace = Path(__file__).resolve().parent.parent
qt = Path(os.environ["QT_ROOT_DIR"]).resolve()
prefix = workspace / "target" / "ci-deps"
kde_version = os.environ["KDE_VERSION"]
qmake = qt / "bin" / ("qmake.exe" if os.name == "nt" else "qmake")

# Kirigami is a runtime QML import, so cargo alone cannot install it.
if not (prefix / "qml" / "org" / "kde" / "kirigami" / "qmldir").is_file():
    for project in ("extra-cmake-modules", "kirigami"):
        source = workspace / "target" / "ci-sources" / project
        build = workspace / "target" / "ci-build" / project
        run(
            "git", "clone", "--depth", "1", "--branch", f"v{kde_version}",
            f"https://github.com/KDE/{project}.git", str(source),
        )
        run(
            "cmake", "-S", str(source), "-B", str(build), "-G", "Ninja",
            "-DCMAKE_BUILD_TYPE=Release",
            f"-DCMAKE_INSTALL_PREFIX={prefix.as_posix()}",
            f"-DCMAKE_PREFIX_PATH={prefix.as_posix()};{qt.as_posix()}",
            "-DKDE_INSTALL_USE_QT_SYS_PATHS=OFF",
            "-DKDE_INSTALL_LIBDIR=lib", "-DKDE_INSTALL_QMLDIR=qml",
            "-DBUILD_TESTING=OFF", "-DBUILD_EXAMPLES=OFF", "-DBUILD_QCH=OFF",
        )
        run("cmake", "--build", str(build), "--parallel", "3")
        run("cmake", "--install", str(build))

environment = {
    "QMAKE": str(qmake),
    "QML_IMPORT_PATH": os.pathsep.join((str(prefix / "qml"), str(qt / "qml"))),
    "QML2_IMPORT_PATH": os.pathsep.join((str(prefix / "qml"), str(qt / "qml"))),
}
if os.name != "nt":
    library_variable = "DYLD_LIBRARY_PATH" if os.uname().sysname == "Darwin" else "LD_LIBRARY_PATH"
    environment[library_variable] = os.pathsep.join(
        (str(prefix / "lib"), str(qt / "lib"), os.environ.get(library_variable, ""))
    )
with open(os.environ["GITHUB_ENV"], "a", encoding="utf-8") as output:
    for key, value in environment.items():
        output.write(f"{key}={value}\n")
with open(os.environ["GITHUB_PATH"], "a", encoding="utf-8") as output:
    output.write(f"{prefix / 'bin'}\n")

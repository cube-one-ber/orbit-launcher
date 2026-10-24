"""Check release completeness, checksums, and publication failure behavior."""

import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock
import zipfile


SCRIPTS = Path(__file__).resolve().parents[1]


def module(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    result = importlib.util.module_from_spec(spec)
    with mock.patch.object(sys, "path", [str(SCRIPTS), *sys.path]):
        spec.loader.exec_module(result)
    return result


release = module("publish-prerelease")
packaging = module("ci-archive")
linux_runtime = module("linux_runtime")
verification = module("check-package")


class ReleaseTests(unittest.TestCase):
    def linux_archive(self, root, libraries=()):
        stage = root / "orbit-linux-x86_64"
        (stage / "lib").mkdir(parents=True)
        (stage / "BUILD-INFO.json").write_text(json.dumps({"commit":"c" * 40, "platform":"linux-x86_64"}))
        (stage / "THIRD-PARTY-NOTICES.md").write_text("notices")
        for name in libraries:
            (stage / "lib" / name).write_bytes(b"library")
        return packaging.archive_stage(stage, root / "artifacts", "linux-x86_64")

    def test_package_check_rejects_host_library_overrides_before_startup(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = self.linux_archive(Path(temporary), ["libwayland-client.so.0"])
            with mock.patch.dict(os.environ, GITHUB_SHA="c" * 40), \
                 mock.patch.object(sys, "argv", ["check-package", "linux-x86_64", str(archive)]), \
                 mock.patch.object(verification.subprocess, "run") as run:
                with self.assertRaisesRegex(RuntimeError, "overrides host graphics/font"):
                    verification.main()
                run.assert_not_called()

    def test_package_native_check_requires_actual_opengl_rendering(self):
        with tempfile.TemporaryDirectory() as temporary:
            archive = self.linux_archive(Path(temporary))
            responses = [subprocess.CompletedProcess([], 0, "ORBIT_UI_TEST_PASS", ""),
                         subprocess.CompletedProcess([], 0, "Using software renderer", "")]
            with mock.patch.dict(os.environ, GITHUB_SHA="c" * 40, QT_QUICK_BACKEND="software"), \
                 mock.patch.object(sys, "argv", ["check-package", "linux-x86_64", str(archive)]), \
                 mock.patch.object(verification.subprocess, "run", side_effect=responses) as run:
                with self.assertRaisesRegex(RuntimeError, "did not exercise the OpenGL renderer"):
                    verification.main()
                native = run.call_args.kwargs["env"]
                self.assertNotIn("QT_QUICK_BACKEND", native)
                self.assertEqual(native["QSG_RHI_BACKEND"], "opengl")

    def test_linux_host_runtime_is_not_bundled(self):
        for name in ("ld-linux-x86-64.so.2", "libc.so.6", "libm.so.6", "libnss_dns.so.2",
                     "libEGL.so.1", "libGLX.so.0", "libGLdispatch.so.0", "libOpenGL.so.0",
                     "libgbm.so.1", "libdrm.so.2", "libdrm_amdgpu.so.1", "libvulkan.so.1",
                     "libwayland-client.so.0", "libwayland-egl.so.1", "libwayland-cursor.so.0",
                     "libfontconfig.so.1", "libfreetype.so.6", "libharfbuzz.so.0", "libexpat.so.1",
                     "libstdc++.so.6", "libgcc_s.so.1", "libgomp.so.1", "libX11.so.6", "libxcb.so.1"):
            with self.subTest(library=name):
                self.assertTrue(linux_runtime.host_library(name))
        for name in ("libQt6Core.so.6", "libQt6OpenGL.so.6", "libQt6WaylandClient.so.6",
                     "libKirigami.so.6", "libglib-2.0.so.0", "libgobject-2.0.so.0",
                     "libxcb-cursor.so.0", "libicudata.so.73", "libgssapi_krb5.so.2"):
            with self.subTest(library=name):
                self.assertFalse(linux_runtime.host_library(name))

    def test_linux_dependency_walk_leaves_driver_and_font_runtime_on_host(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            stage, qt, kde = (root / name for name in ("stage", "qt", "kde"))
            (stage / "bin").mkdir(parents=True)
            (stage / "bin/orbit").write_bytes(b"executable")
            for prefix in (qt, kde):
                (prefix / "lib").mkdir(parents=True)
                (prefix / "qml").mkdir()
            # SDK copies must also obey the boundary, not just ldd dependencies.
            (qt / "lib/libwayland-client.so.0").write_bytes(b"old Wayland")
            (qt / "lib/libfontconfig.so.1").write_bytes(b"old font rules parser")
            (qt / "lib/libQt6Core.so.6").write_bytes(b"Qt")
            runtime = root / "libicudata.so.73"
            runtime.write_bytes(b"ICU")
            def run(*args, **kwargs):
                if args[0] == "ldd":
                    return subprocess.CompletedProcess(args, 0, f"libwayland-client.so.0 => /host/libwayland-client.so.0 (0)\nlibEGL.so.1 => /host/libEGL.so.1 (0)\nlibfontconfig.so.1 => /host/libfontconfig.so.1 (0)\nlibicudata.so.73 => {runtime} (0)\n")
                return subprocess.CompletedProcess(args, 0)
            with mock.patch.object(packaging, "run", side_effect=run):
                packaging.linux_runtime(stage, qt, kde)
            self.assertEqual(sorted(p.name for p in (stage / "lib").iterdir()),
                             ["libQt6Core.so.6", "libicudata.so.73"])

    def test_windows_archive_accepts_cargo_license_timestamps(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            stage = directory / "orbit-windows-x86_64"
            stage.mkdir()
            license = stage / "LICENSE"
            license.write_bytes(b"license contents")
            os.utime(license, (0, 0))
            path = packaging.archive_stage(stage, directory / "artifacts", "windows-x86_64")
            with zipfile.ZipFile(path) as archive:
                self.assertEqual(archive.read("orbit-windows-x86_64/LICENSE"), b"license contents")
                self.assertEqual(archive.getinfo("orbit-windows-x86_64/LICENSE").date_time[0], 1980)

    def prepare(self, directory):
        for name in release.EXPECTED_ASSETS:
            (directory / name).write_bytes(f"payload for {name}".encode())

    def test_incomplete_release_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.prepare(directory)
            (directory / "orbit-windows-x86_64.zip").unlink()
            with self.assertRaisesRegex(RuntimeError, "orbit-windows-x86_64.zip"):
                release.prepare_assets(directory)
            self.assertFalse((directory / "SHA256SUMS").exists())

    def test_checksums_cover_all_downloads(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.prepare(directory)
            assets = release.prepare_assets(directory)
            self.assertEqual(len(assets), len(release.EXPECTED_ASSETS) + 1)
            for line in (directory / "SHA256SUMS").read_text().splitlines():
                digest, name = line.split("  ")
                self.assertEqual(digest, hashlib.sha256((directory / name).read_bytes()).hexdigest())

    def test_failed_upload_cannot_publish_new_release(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.prepare(directory)
            responses = [subprocess.CompletedProcess([], 1), subprocess.CalledProcessError(1, "gh")]
            with mock.patch.dict(os.environ, GITHUB_SHA="a" * 40, GITHUB_REPOSITORY="owner/repo", GITHUB_REF_NAME="master"), \
                 mock.patch("sys.argv", ["publish-prerelease", str(directory)]), \
                 mock.patch.object(release.subprocess, "run", side_effect=responses) as run:
                with self.assertRaises(subprocess.CalledProcessError):
                    release.main()
                self.assertEqual(run.call_count, 2)
                self.assertIn("--draft", run.call_args.args[0])
                self.assertIn("--target", run.call_args.args[0])
                self.assertNotIn("edit", run.call_args.args[0])

    def test_rerun_reuses_commit_tag(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            self.prepare(directory)
            with mock.patch.dict(os.environ, GITHUB_SHA="b" * 40, GITHUB_REPOSITORY="owner/repo", GITHUB_REF_NAME="master"), \
                 mock.patch("sys.argv", ["publish-prerelease", str(directory)]), \
                 mock.patch.object(release.subprocess, "run", return_value=subprocess.CompletedProcess([], 0)) as run:
                release.main()
                commands = [call.args[0] for call in run.call_args_list]
                self.assertEqual([command[2] for command in commands], ["view", "upload", "edit"])
                self.assertTrue(all(command[3] == "ci-" + "b" * 40 for command in commands))
                self.assertIn("--clobber", commands[1])
                self.assertIn("--latest=false", commands[2])


if __name__ == "__main__":
    unittest.main()

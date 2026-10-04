"""Check release completeness, checksums, and publication failure behavior."""

import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock


SCRIPTS = Path(__file__).resolve().parents[1]


def module(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


release = module("publish-prerelease")


class ReleaseTests(unittest.TestCase):
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

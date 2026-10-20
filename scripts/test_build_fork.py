"""Behavioral tests for the fork's thin Cargo/V8 entry point (no builds)."""

import os
import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import build_fork
from codex_package import v8


class ForkBuildTest(unittest.TestCase):
    def setUp(self):
        self.environ = patch.dict(os.environ, {}, clear=True)
        self.environ.start()
        self.addCleanup(self.environ.stop)
        self.run = patch.object(build_fork.subprocess, "run").start()
        self.addCleanup(patch.stopall)
        self.run.side_effect = self.process
        self.resolve = patch.object(
            build_fork,
            "resolve_codex_v8_cargo_env",
            return_value={
                "RUSTY_V8_ARCHIVE": "/cache/archive.a.gz",
                "RUSTY_V8_SRC_BINDING_PATH": "/cache/binding.rs",
            },
        ).start()

    def process(self, command, **kwargs):
        return subprocess.CompletedProcess(command, 0, "aarch64-apple-darwin\n")

    def cargo_call(self):
        return [call for call in self.run.call_args_list if call.args[0][0] == "cargo"][
            0
        ]

    def test_default_resolves_host_and_preserves_explicit_target_cache(self):
        self.assertEqual(build_fork.main([]), 0)
        self.assertEqual(self.resolve.call_args.args[0].target, "aarch64-apple-darwin")
        call = self.cargo_call()
        self.assertEqual(call.kwargs["env"]["CARGO_BUILD_TARGET"], "host-tuple")
        self.assertEqual(
            call.args[0],
            [
                "cargo",
                "build",
                "--release",
                "--locked",
                "--timings",
                "-p",
                "codex-cli",
                "-p",
                "codex-code-mode-host",
                "--bin",
                "codex",
                "--bin",
                "codex-code-mode-host",
            ],
        )
        self.assertEqual(call.kwargs["env"]["RUSTY_V8_ARCHIVE"], "/cache/archive.a.gz")
        self.assertEqual(call.kwargs["cwd"], build_fork.REPO_ROOT / "codex-rs")
        self.assertNotIn("RUSTY_V8_ARCHIVE", os.environ)
        self.assertNotIn("CARGO_BUILD_TARGET", os.environ)

    def test_environment_target_selects_matching_artifact(self):
        os.environ["CARGO_BUILD_TARGET"] = "x86_64-unknown-linux-gnu"
        build_fork.main([])
        self.assertEqual(
            self.resolve.call_args.args[0].target, "x86_64-unknown-linux-gnu"
        )
        self.assertEqual(len(self.run.call_args_list), 1)

    def test_cli_target_overrides_environment_in_both_forms(self):
        os.environ["CARGO_BUILD_TARGET"] = "aarch64-apple-darwin"
        for args in (
            ["--target", "x86_64-apple-darwin"],
            ["--target=x86_64-apple-darwin"],
        ):
            with self.subTest(args=args):
                build_fork.main(args)
                self.assertEqual(
                    self.resolve.call_args.args[0].target, "x86_64-apple-darwin"
                )
                self.assertEqual(self.cargo_call().args[0][-len(args) :], args)
                self.run.reset_mock()

    def test_unsupported_target_fails_before_download(self):
        self.assertNotEqual(build_fork.main(["--target=unsupported-triple"]), 0)
        self.resolve.assert_not_called()
        self.run.assert_not_called()

    def test_host_detection_failure_never_prepares_or_builds(self):
        self.run.side_effect = subprocess.CalledProcessError(7, ["rustc"])
        self.assertNotEqual(build_fork.main([]), 0)
        self.resolve.assert_not_called()
        self.assertEqual(self.run.call_count, 1)

    def test_argument_spaces_survive(self):
        args = ["--target-dir", "/a directory/with spaces"]
        build_fork.main(args)
        self.assertEqual(self.cargo_call().args[0][-2:], args)

    def test_preparation_failure_never_launches_cargo(self):
        self.resolve.side_effect = RuntimeError("checksum mismatch")
        self.assertNotEqual(build_fork.main([]), 0)
        self.assertFalse(
            any(call.args[0][0] == "cargo" for call in self.run.call_args_list)
        )

    def test_cargo_exit_status_is_preserved(self):
        self.run.side_effect = lambda command, **kwargs: subprocess.CompletedProcess(
            command, 17 if command[0] == "cargo" else 0, "aarch64-apple-darwin\n"
        )
        self.assertEqual(build_fork.main([]), 17)

    def test_multiple_targets_are_rejected_before_preparation(self):
        for args in (
            ["--target=aarch64-apple-darwin", "--target=x86_64-apple-darwin"],
            ["--target=aarch64-apple-darwin,x86_64-apple-darwin"],
            ["--target"],
        ):
            with self.subTest(args=args):
                self.assertNotEqual(build_fork.main(args), 0)
        self.resolve.assert_not_called()
        self.run.assert_not_called()

    def test_real_helper_rejects_single_override_without_download_or_cargo(self):
        self.resolve.side_effect = v8.resolve_codex_v8_cargo_env
        os.environ["RUSTY_V8_ARCHIVE"] = "/custom/archive"
        with patch.object(v8, "fetch_codex_v8_artifacts") as fetch:
            self.assertNotEqual(build_fork.main([]), 0)
        fetch.assert_not_called()
        self.assertFalse(
            any(call.args[0][0] == "cargo" for call in self.run.call_args_list)
        )

    def test_real_helper_preserves_pair_overrides_and_source_build(self):
        self.resolve.side_effect = v8.resolve_codex_v8_cargo_env
        for env in (
            {
                "RUSTY_V8_ARCHIVE": "/custom/archive",
                "RUSTY_V8_SRC_BINDING_PATH": "/custom/binding",
            },
            {"V8_FROM_SOURCE": "1"},
        ):
            with (
                self.subTest(env=env),
                patch.dict(os.environ, env, clear=True),
                patch.object(v8, "fetch_codex_v8_artifacts") as fetch,
            ):
                self.assertEqual(build_fork.main([]), 0)
                for key, value in env.items():
                    self.assertEqual(self.cargo_call().kwargs["env"][key], value)
                fetch.assert_not_called()
                self.run.reset_mock()


if __name__ == "__main__":
    unittest.main()

"""Prepare Codex's sandbox V8 artifacts, then build the CLI and host."""

from __future__ import annotations

import os
from pathlib import Path
import subprocess
import sys

# Keep the local helpers importable even with PYTHONSAFEPATH enabled.
REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "scripts"))
os.environ.setdefault("CODEX_REPO_ROOT", str(REPO_ROOT))

from codex_package.targets import TARGET_SPECS  # noqa: E402
from codex_package.v8 import resolve_codex_v8_cargo_env  # noqa: E402


def effective_target(args: list[str], environ: dict[str, str]) -> str:
    targets = []
    arguments = iter(args)
    for argument in arguments:
        if argument == "--target":
            target = next(arguments, "")
            if not target or target.startswith("-"):
                raise ValueError("--target needs a target triple")
            targets.append(target)
        elif argument.startswith("--target="):
            targets.append(argument.removeprefix("--target="))
    if len(targets) > 1:
        raise ValueError("Build one target at a time so V8 artifacts match the target")
    target = targets[0] if targets else environ["CARGO_BUILD_TARGET"]
    if not target or "," in target:
        raise ValueError("Build one target triple at a time")
    if target == "host-tuple":
        target = subprocess.run(
            ["rustc", "--print", "host-tuple"],
            cwd=REPO_ROOT / "codex-rs",
            env=environ,
            check=True,
            capture_output=True,
            text=True,
        ).stdout.strip()
    if target not in TARGET_SPECS:
        raise ValueError(f"No Codex V8 artifact configuration for target: {target}")
    return target


def main(args: list[str] | None = None) -> int:
    args = sys.argv[1:] if args is None else args
    environ = dict(os.environ)
    environ.setdefault("CARGO_BUILD_TARGET", "host-tuple")
    try:
        target = effective_target(args, environ)
        environ.update(
            resolve_codex_v8_cargo_env(TARGET_SPECS[target], environ=environ)
        )
        return subprocess.run(
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
                *args,
            ],
            cwd=REPO_ROOT / "codex-rs",
            env=environ,
        ).returncode
    except (OSError, RuntimeError, ValueError, subprocess.SubprocessError) as error:
        print(f"Codex build preparation failed: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())

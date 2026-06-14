#!/usr/bin/env python3
"""Download pre-built native binaries from a GitHub Release workflow artifact.

This is a fallback for local staging when native components need to be fetched
from a CI workflow run. In CI, binaries are already available from cargo build,
so this is mainly for local manual releases via stage_npm_packages.py.
"""

from __future__ import annotations

import argparse
import subprocess
import tempfile
from pathlib import Path

GITHUB_REPO = "openagere/openagere"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--workflow-url",
        required=True,
        help="URL of the GitHub Actions workflow run that produced the artifacts.",
    )
    parser.add_argument(
        "--component",
        dest="components",
        action="append",
        required=True,
        help="Component name to download (openagere). May be specified multiple times.",
    )
    parser.add_argument(
        "output_dir",
        type=Path,
        help="Directory to download binaries into.",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    args.output_dir.mkdir(parents=True, exist_ok=True)

    # Extract workflow run ID from URL
    # URL format: https://github.com/owner/repo/actions/runs/1234567890
    run_id = args.workflow_url.rstrip("/").split("/")[-1]
    if not run_id.isdigit():
        raise ValueError(f"Could not extract run ID from URL: {args.workflow_url}")

    print(f"Downloading components: {', '.join(args.components)}")
    print(f"From workflow run: {run_id}")
    print(f"To: {args.output_dir}")

    for component in args.components:
        if component == "openagere":
            artifact_name = "openagere-binaries"
        else:
            print(f"Warning: unknown component '{component}', skipping")
            continue

        with tempfile.TemporaryDirectory() as tmp:
            tmp_path = Path(tmp)
            archive = tmp_path / "artifact.zip"

            try:
                subprocess.run(
                    [
                        "gh", "api",
                        f"repos/{GITHUB_REPO}/actions/runs/{run_id}/artifacts",
                        "--jq",
                        f'.artifacts[] | select(.name == "{artifact_name}") | .archive_download_url',
                        "--output", str(tmp_path / "url.txt"),
                    ],
                    check=True,
                    capture_output=True,
                )
            except subprocess.CalledProcessError:
                print(f"Warning: could not query artifacts for {artifact_name}")
                continue

            url_file = tmp_path / "url.txt"
            if not url_file.exists() or not url_file.read_text().strip():
                print(f"Warning: artifact '{artifact_name}' not found")
                continue

            try:
                subprocess.run(
                    ["gh", "api", url_file.read_text().strip(), "--output", str(archive)],
                    check=True,
                    capture_output=True,
                )
            except subprocess.CalledProcessError:
                print(f"Warning: could not download artifact {artifact_name}")
                continue

            subprocess.run(
                ["unzip", "-o", str(archive), "-d", str(args.output_dir)],
                check=True,
                capture_output=True,
            )

        print(f"Downloaded {component} to {args.output_dir}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())

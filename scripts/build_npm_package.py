#!/usr/bin/env python3
"""Build a single Openagere npm package (main or platform-specific) for release."""

from __future__ import annotations

import argparse
import json
import shutil
import tarfile
from pathlib import Path

SCRIPTS_DIR = Path(__file__).resolve().parent
NPM_DIR = SCRIPTS_DIR / "npm"
REPO_ROOT = SCRIPTS_DIR.parent

# Platform -> (os, cpu, rust_target_triple) mapping
PLATFORM_INFO = {
    "darwin-x64": ("darwin", "x64", "x86_64-apple-darwin"),
    "darwin-arm64": ("darwin", "arm64", "aarch64-apple-darwin"),
    "linux-x64": ("linux", "x64", "x86_64-unknown-linux-musl"),
    "linux-arm64": ("linux", "arm64", "aarch64-unknown-linux-musl"),
    "win32-x64": ("win32", "x64", "x86_64-pc-windows-msvc"),
    "win32-arm64": ("win32", "arm64", "aarch64-pc-windows-msvc"),
}

# Which native components each package needs (only openagere binary)
PACKAGE_NATIVE_COMPONENTS = {
    "openagere": {"openagere"},
}

# Package name expansions: "openagere" expands to all platform packages
# Using npm: alias pattern - same package name with version suffix
PACKAGE_EXPANSIONS = {
    "openagere": [f"openagere-{p}" for p in PLATFORM_INFO.keys()],
}

# Set of platform package identifiers
OPENAGERE_PLATFORM_PACKAGES = {f"openagere-{p}" for p in PLATFORM_INFO.keys()}


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--package",
        required=True,
        help="Package name to build (e.g. openagere or openagere-linux-x64).",
    )
    parser.add_argument(
        "--release-version",
        required=True,
        help="Version string (e.g. 0.1.0).",
    )
    parser.add_argument(
        "--staging-dir",
        required=True,
        type=Path,
        help="Directory to assemble the package in.",
    )
    parser.add_argument(
        "--pack-output",
        required=True,
        type=Path,
        help="Where to write the resulting .tgz.",
    )
    parser.add_argument(
        "--vendor-src",
        type=Path,
        default=None,
        help="Directory containing pre-built native binaries.",
    )
    parser.add_argument(
        "--rust-target",
        type=str,
        default=None,
        help="Rust target triple (required for platform packages).",
    )
    parser.add_argument(
        "--agere-bin",
        type=Path,
        default=None,
        help="Path to the compiled openagere binary (required for platform packages).",
    )
    parser.add_argument(
        "--rg-bin",
        type=Path,
        default=None,
        help="Path to ripgrep binary to bundle in vendor directory.",
    )
    return parser.parse_args()


def build_main_package(args: argparse.Namespace) -> None:
    """Build the main openagere package (JS entry point only)."""
    staging = args.staging_dir
    bin_dir = staging / "bin"
    bin_dir.mkdir(parents=True)

    # Copy openagere.js
    src = NPM_DIR / "bin" / "openagere.js"
    shutil.copy2(src, bin_dir / "openagere.js")

    # Copy postinstall.js
    postinstall_src = NPM_DIR / "bin" / "postinstall.js"
    if postinstall_src.exists():
        shutil.copy2(postinstall_src, bin_dir / "postinstall.js")

    # Copy README
    readme = NPM_DIR / "README.md"
    if readme.exists():
        shutil.copy2(readme, staging / "README.md")

    # Generate package.json from template
    template = (NPM_DIR / "package-main.json").read_text()
    version = args.release_version
    package_json = template.replace("{{VERSION}}", version)
    (staging / "package.json").write_text(package_json)


def build_platform_package(args: argparse.Namespace) -> None:
    """Build a platform-specific package containing the native binary.
    
    Uses the same package name "openagere" with a version suffix (e.g. 0.1.0-linux-x64),
    following the same pattern as @openai/codex for reliable npm optional dependency resolution.
    """
    if args.package not in OPENAGERE_PLATFORM_PACKAGES:
        raise ValueError(f"Unknown platform package: {args.package}")

    platform_key = args.package.removeprefix("openagere-")
    if platform_key not in PLATFORM_INFO:
        raise ValueError(f"Unknown platform key: {platform_key}")

    os_name, cpu_name, target_triple = PLATFORM_INFO[platform_key]

    if args.rust_target and args.rust_target != target_triple:
        raise ValueError(
            f"rust-target mismatch: expected {target_triple}, got {args.rust_target}"
        )

    staging = args.staging_dir
    vendor_dir = staging / "vendor" / target_triple / "bin"
    vendor_dir.mkdir(parents=True)

    # Copy openagere binary
    if args.agere_bin is None:
        raise ValueError("--agere-bin is required for platform packages")
    bin_name = "openagere.exe" if os_name == "win32" else "openagere"
    shutil.copy2(args.agere_bin, vendor_dir / bin_name)
    # Ensure executable bit
    (vendor_dir / bin_name).chmod(0o755)

    # Copy bundled ripgrep
    if args.rg_bin:
        rg_name = "rg.exe" if os_name == "win32" else "rg"
        # Copy rg to a separate 'path' directory so the Node.js launcher
        # prepends it to PATH and makes rg available to shell commands.
        path_dir = staging / "vendor" / target_triple / "path"
        path_dir.mkdir(parents=True, exist_ok=True)
        shutil.copy2(args.rg_bin, path_dir / rg_name)
        (path_dir / rg_name).chmod(0o755)

    # Copy README if available
    readme = NPM_DIR / "README.md"
    if readme.exists():
        shutil.copy2(readme, staging / "README.md")

    # Generate package.json directly (no template needed)
    # Same package name "openagere" with version suffix, os/cpu constraints
    package_json = json.dumps({
        "name": "openagere",
        "version": f"{args.release_version}-{platform_key}",
        "description": f"Openagere CLI native binary for {platform_key}",
        "os": [os_name],
        "cpu": [cpu_name],
        "preferUnplugged": True,
        "bin": {
            "openagere": f"vendor/{target_triple}/bin/{bin_name}"
        },
        "files": ["vendor", "README.md"],
        "license": "Apache-2.0",
    }, indent=2)
    (staging / "package.json").write_text(package_json)


def create_tarball(staging_dir: Path, output_path: Path) -> None:
    """Create a .tgz archive from the staging directory."""
    output_path.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(output_path, "w:gz") as tar:
        tar.add(staging_dir, arcname="package")


def main() -> int:
    args = parse_args()

    if args.package == "openagere":
        build_main_package(args)
    else:
        build_platform_package(args)

    create_tarball(args.staging_dir, args.pack_output)
    print(f"Built {args.pack_output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

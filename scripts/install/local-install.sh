#!/bin/sh
# Compile openagere from source and replace the npm-managed binary.
#
# Usage: ./scripts/install/local-install.sh [--uninstall]

set -eu

INSTALL_DIR="${OPENAGERE_INSTALL_DIR:-$HOME/.local/bin}"
BIN_PATH="$INSTALL_DIR/openagere"
INSTALL_MODE="install"

step() {
  printf '==> %s\n' "$1" >&2
}

warn() {
  printf 'WARNING: %s\n' "$1" >&2
}

uninstall() {
  if [ -f "$BIN_PATH" ] || [ -L "$BIN_PATH" ]; then
    step "Removing $BIN_PATH"
    rm -f "$BIN_PATH"
  else
    step "No installation found at $BIN_PATH"
  fi
}

detect_target_triple() {
  os_name=$(uname -s)
  arch_name=$(uname -m)
  case "$os_name:$arch_name" in
    Darwin:x86_64) printf '%s\n' 'x86_64-apple-darwin' ;;
    Darwin:arm64) printf '%s\n' 'aarch64-apple-darwin' ;;
    Linux:x86_64) printf '%s\n' 'x86_64-unknown-linux-musl' ;;
    Linux:aarch64|Linux:arm64) printf '%s\n' 'aarch64-unknown-linux-musl' ;;
    *)
      printf 'Unsupported platform for npm binary replacement: %s %s\n' "$os_name" "$arch_name" >&2
      return 1
      ;;
  esac
}

platform_package_for_target() {
  case "$1" in
    x86_64-unknown-linux-musl) printf '%s\n' '@openagere/openagere-linux-x64' ;;
    aarch64-unknown-linux-musl) printf '%s\n' '@openagere/openagere-linux-arm64' ;;
    x86_64-apple-darwin) printf '%s\n' '@openagere/openagere-darwin-x64' ;;
    aarch64-apple-darwin) printf '%s\n' '@openagere/openagere-darwin-arm64' ;;
    *)
      printf 'Unsupported target triple for npm binary replacement: %s\n' "$1" >&2
      return 1
      ;;
  esac
}

find_npm_package_root() {
  if [ -n "${OPENAGERE_NPM_PACKAGE_DIR:-}" ]; then
    printf '%s\n' "$OPENAGERE_NPM_PACKAGE_DIR"
    return
  fi

  if command -v npm >/dev/null 2>&1; then
    npm_root=$(npm root -g 2>/dev/null || true)
    if [ -n "$npm_root" ] && [ -f "$npm_root/openagere/package.json" ]; then
      printf '%s\n' "$npm_root/openagere"
      return
    fi
  fi

  if command -v openagere >/dev/null 2>&1; then
    shim_path=$(command -v openagere)
    shim_dir=$(cd "$(dirname "$shim_path")" && pwd)
    if [ -f "$shim_dir/node_modules/openagere/package.json" ]; then
      printf '%s\n' "$shim_dir/node_modules/openagere"
      return
    fi
    nvm_candidate="$(cd "$shim_dir/.." && pwd)/lib/node_modules/openagere"
    if [ -f "$nvm_candidate/package.json" ]; then
      printf '%s\n' "$nvm_candidate"
      return
    fi
  fi

  printf 'Could not find a global npm installation of openagere. Run `npm install -g openagere` first, or set OPENAGERE_NPM_PACKAGE_DIR.\n' >&2
  return 1
}

find_npm_vendor_binary() {
  if [ -n "${OPENAGERE_NPM_BINARY:-}" ]; then
    printf '%s\n' "$OPENAGERE_NPM_BINARY"
    return
  fi

  target_triple=$(detect_target_triple)
  package_root=$(find_npm_package_root)
  package_parent=$(cd "$package_root/.." && pwd)
  platform_package=$(platform_package_for_target "$target_triple")
  platform_package_path="$package_parent/$platform_package"
  binary_path="$platform_package_path/vendor/$target_triple/bin/openagere"
  if [ -f "$binary_path" ]; then
    printf '%s\n' "$binary_path"
    return
  fi

  nested_binary_path="$package_root/node_modules/$platform_package/vendor/$target_triple/bin/openagere"
  if [ -f "$nested_binary_path" ]; then
    printf '%s\n' "$nested_binary_path"
    return
  fi

  local_binary_path="$package_root/vendor/$target_triple/bin/openagere"
  if [ -f "$local_binary_path" ]; then
    printf '%s\n' "$local_binary_path"
    return
  fi

  printf 'Could not find npm vendor binary. Checked: %s, %s, and %s\n' "$binary_path" "$nested_binary_path" "$local_binary_path" >&2
  return 1
}

build_openagere() {
  repo_root="$1"
  cores=$(nproc 2>/dev/null || sysctl -n hw.ncpu 2>/dev/null || echo 4)

  step "Building openagere with release profile ($cores parallel jobs)..."
  CARGO_BUILD_JOBS="$cores" \
    cargo build --release --manifest-path "$repo_root/Cargo.toml" -p agere-cli --bin openagere

  printf '%s\n' "$repo_root/target/release/openagere"
}

install() {
  script_dir="$(cd "$(dirname "$0")" && pwd)"
  repo_root="$(cd "$script_dir/../.." && pwd)"
  built_binary=$(build_openagere "$repo_root")
  npm_binary=$(find_npm_vendor_binary)

  step "Replacing npm vendor binary at $npm_binary..."
  cp "$built_binary" "$npm_binary"
  chmod 0755 "$npm_binary"

  step "Done!"
  printf '\n'
  printf 'npm-managed openagere now uses %s\n' "$built_binary"
  printf 'Run: openagere --version\n'
}

while [ $# -gt 0 ]; do
  case "$1" in
    --uninstall)
      INSTALL_MODE="uninstall"
      ;;
    *)
      printf 'Unknown argument: %s\n' "$1" >&2
      exit 2
      ;;
  esac
  shift
done

case "$(uname -s)" in
  Darwin) os="darwin" ;;
  Linux) os="linux" ;;
  *) os="" ;;
esac

case "$INSTALL_MODE" in
  uninstall) uninstall ;;
  install) install ;;
esac

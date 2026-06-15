#!/usr/bin/env node
// Post-install validation script: checks that the platform-specific binary package was installed.

import { existsSync } from "fs";
import { createRequire } from "node:module";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const require = createRequire(import.meta.url);

const PLATFORM_PACKAGE_BY_TARGET = {
  "x86_64-unknown-linux-musl": "@openagere/openagere-linux-x64",
  "aarch64-unknown-linux-musl": "@openagere/openagere-linux-arm64",
  "x86_64-apple-darwin": "@openagere/openagere-darwin-x64",
  "aarch64-apple-darwin": "@openagere/openagere-darwin-arm64",
  "x86_64-pc-windows-msvc": "@openagere/openagere-win32-x64",
  "aarch64-pc-windows-msvc": "@openagere/openagere-win32-arm64",
};

const { platform, arch } = process;

let targetTriple = null;
switch (platform) {
  case "linux":
  case "android":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-unknown-linux-musl";
        break;
      case "arm64":
        targetTriple = "aarch64-unknown-linux-musl";
        break;
    }
    break;
  case "darwin":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-apple-darwin";
        break;
      case "arm64":
        targetTriple = "aarch64-apple-darwin";
        break;
    }
    break;
  case "win32":
    switch (arch) {
      case "x64":
        targetTriple = "x86_64-pc-windows-msvc";
        break;
      case "arm64":
        targetTriple = "aarch64-pc-windows-msvc";
        break;
    }
    break;
}

if (!targetTriple) {
  console.warn(`[openagere] Unsupported platform: ${platform} (${arch})`);
  process.exit(0);
}

const platformPackage = PLATFORM_PACKAGE_BY_TARGET[targetTriple];
const binaryName = process.platform === "win32" ? "openagere.exe" : "openagere";

// Check if the platform package is properly installed
let packageInstalled = false;
try {
  const packageJsonPath = require.resolve(`${platformPackage}/package.json`);
  const packageDir = path.dirname(packageJsonPath);
  const vendorDir = path.join(packageDir, "vendor", targetTriple, "bin", binaryName);
  packageInstalled = existsSync(vendorDir);
} catch {
  packageInstalled = false;
}

// Also check local vendor (for development setups)
const localBinaryPath = path.join(__dirname, "..", "vendor", targetTriple, "bin", binaryName);
const hasLocalBinary = existsSync(localBinaryPath);

if (!packageInstalled && !hasLocalBinary) {
  console.error(``);
  console.error(`[openagere] Warning: Platform package ${platformPackage} is not installed.`);
  console.error(``);
  console.error(`This package provides the native binary for your platform (${process.platform}/${process.arch}).`);
  console.error(`npm may have skipped it because it is listed as an optional dependency.`);
  console.error(``);
  console.error(`To fix this, run:`);
  console.error(`  npm install -g ${platformPackage}`);
  console.error(`  npm install -g openagere@latest`);
  console.error(``);
  console.error(`If the issue persists, try:`);
  console.error(`  npm config set optional true`);
  console.error(`  npm install -g openagere@latest`);
  console.error(``);
  process.exit(0);
} else {
  console.log(`[openagere] Native binary for ${platform}/${arch} installed successfully.`);
}
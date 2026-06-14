#!/usr/bin/env node
// Unified entry point for the Openagere CLI.

import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

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
      default:
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
      default:
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
      default:
        break;
    }
    break;
  default:
    break;
}

if (!targetTriple) {
  throw new Error(`Unsupported platform: ${platform} (${arch})`);
}

const platformPackage = PLATFORM_PACKAGE_BY_TARGET[targetTriple];
if (!platformPackage) {
  throw new Error(`Unsupported target triple: ${targetTriple}`);
}

const binaryName = process.platform === "win32" ? "openagere.exe" : "openagere";
const localVendorRoot = path.join(__dirname, "..", "vendor");
const localBinaryPath = path.join(
  localVendorRoot,
  targetTriple,
  "bin",
  binaryName,
);

/**
 * Locate the package root directory across different install scenarios.
 *
 * npm copies bin files to a separate directory (e.g. PREFIX/bin/) rather than
 * symlinking, so require.resolve called from __dirname cannot reach
 * node_modules where optional dependencies are installed. We try multiple
 * heuristics to find the actual package root.
 */
function findPackageRoot() {
  const parent = path.resolve(__dirname, "..");

  // 1) Direct parent has package.json — local install or symlinked global
  if (existsSync(path.join(parent, "package.json"))) {
    return parent;
  }

  // 2) nvm / fnm global: PREFIX/bin/ → PREFIX/lib/node_modules/openagere/
  const nvmCandidate = path.join(parent, "lib", "node_modules", "openagere");
  if (existsSync(path.join(nvmCandidate, "package.json"))) {
    return nvmCandidate;
  }

  // 3) Volta global: ~/.volta/bin/ → ~/.volta/tools/image/packages/openagere/
  const voltaBase = path.resolve(os.homedir(), ".volta");
  if (__dirname.startsWith(voltaBase)) {
    const voltaCandidate = path.join(
      voltaBase,
      "tools",
      "image",
      "packages",
      "openagere",
    );
    if (existsSync(path.join(voltaCandidate, "package.json"))) {
      return voltaCandidate;
    }
  }

  // 4) Fallback to direct parent — require.resolve may still find hoisted deps
  return parent;
}

const packageRoot = findPackageRoot();
const packageRootRequire = createRequire(path.join(packageRoot, "package.json"));

let vendorRoot;
try {
  const packageJsonPath = packageRootRequire.resolve(`${platformPackage}/package.json`);
  vendorRoot = path.join(path.dirname(packageJsonPath), "vendor");
} catch {
  if (existsSync(localBinaryPath)) {
    vendorRoot = localVendorRoot;
  } else {
    const packageManager = detectPackageManager();
    const updateCommand =
      packageManager === "bun"
        ? "bun add -g openagere@latest"
        : "npm install -g openagere@latest";
    throw new Error(
      `Missing optional dependency ${platformPackage}. Reinstall Openagere: ${updateCommand}`,
    );
  }
}

if (!vendorRoot) {
  const packageManager = detectPackageManager();
  const updateCommand =
    packageManager === "bun"
      ? "bun add -g openagere@latest"
      : "npm install -g openagere@latest";
  throw new Error(
    `Missing optional dependency ${platformPackage}. Reinstall Openagere: ${updateCommand}`,
  );
}

const archRoot = path.join(vendorRoot, targetTriple);
const binaryPath = path.join(archRoot, "bin", binaryName);

function getUpdatedPath(newDirs) {
  const pathSep = process.platform === "win32" ? ";" : ":";
  const existingPath = process.env.PATH || "";
  const updatedPath = [
    ...newDirs,
    ...existingPath.split(pathSep).filter(Boolean),
  ].join(pathSep);
  return updatedPath;
}

/**
 * Use heuristics to detect the package manager that was used to install Openagere
 * in order to give the user a hint about how to update it.
 */
function detectPackageManager() {
  const userAgent = process.env.npm_config_user_agent || "";
  if (/\bbun\//.test(userAgent)) {
    return "bun";
  }

  const execPath = process.env.npm_execpath || "";
  if (execPath.includes("bun")) {
    return "bun";
  }

  if (
    __dirname.includes(".bun/install/global") ||
    __dirname.includes(".bun\\install\\global")
  ) {
    return "bun";
  }

  return userAgent ? "npm" : null;
}

const additionalDirs = [];
const pathDir = path.join(archRoot, "path");
if (existsSync(pathDir)) {
  additionalDirs.push(pathDir);
}
const updatedPath = getUpdatedPath(additionalDirs);

const env = { ...process.env, PATH: updatedPath };
const packageManagerEnvVar =
  detectPackageManager() === "bun"
    ? "OPENAGERE_MANAGED_BY_BUN"
    : "OPENAGERE_MANAGED_BY_NPM";
env[packageManagerEnvVar] = "1";

const child = spawn(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  env,
});

child.on("error", (err) => {
  // Typically triggered when the binary is missing or not executable.
  // Re-throwing here will terminate the parent with a non-zero exit code
  // while still printing a helpful stack trace.
  console.error(err);
  process.exit(1);
});

// Forward common termination signals to the child so that it shuts down
// gracefully. In the handler we temporarily disable the default behavior of
// exiting immediately; once the child has been signaled we simply wait for
// its exit event which will in turn terminate the parent (see below).
const forwardSignal = (signal) => {
  if (child.killed) {
    return;
  }
  try {
    child.kill(signal);
  } catch {
    /* ignore */
  }
};

["SIGINT", "SIGTERM", "SIGHUP"].forEach((sig) => {
  process.on(sig, () => forwardSignal(sig));
});

// When the child exits, mirror its termination reason in the parent so that
// shell scripts and other tooling observe the correct exit status.
// Wrap the lifetime of the child process in a Promise so that we can await
// its termination in a structured way. The Promise resolves with an object
// describing how the child exited: either via exit code or due to a signal.
const childResult = await new Promise((resolve) => {
  child.on("exit", (code, signal) => {
    if (signal) {
      resolve({ type: "signal", signal });
    } else {
      resolve({ type: "code", exitCode: code ?? 1 });
    }
  });
});

if (childResult.type === "signal") {
  // Re-emit the same signal so that the parent terminates with the expected
  // semantics (this also sets the correct exit code of 128 + n).
  process.kill(process.pid, childResult.signal);
} else {
  process.exit(childResult.exitCode);
}

# agere-core

This crate implements the business logic for Agere. It is designed to be used by the various Agere UIs written in Rust.

## Terminology

Sections below sometimes refer to **OS-level isolation** (Seatbelt on macOS,
Linux user namespaces / Landlock / bubblewrap, Windows token and path-based
restriction). That wording is about the platform mechanism. User-facing limits
are modeled by **permission profiles**, **`access_mode`**, and (on the wire)
**`access_policy` / `filesystem_access` / `permission_profile`** fields in the
Access vocabulary; they are not tied to a single OS backend.

## Dependencies

`agere-core` assumes certain helper behavior is available in the environment.
The rough matrix:

### macOS

Managed execution resolves a Seatbelt profile from the active permission profile
(filesystem carve-outs, writable roots, and network posture). Typical
workspace-write setups keep `.git` (directory or gitdir pointer) and `.openagere`
read-only while allowing writes under configured roots.

### Linux

When the process is launched with argv0 **`agere-linux-helper`**, the stack
reattaches through the helper entry used for bubblewrap-backed runs (see the
`agere-arg0` crate). Permission profiles decide whether enforcement uses
Landlock-only paths vs bubblewrap, including legacy-compat routing when policies
still round-trip through older internal representations.

The helper prefers `bwrap` from `PATH` when usable; very old bubblewrap builds
omit `--argv0` and fall back to a compat re-exec. If `bwrap` is unavailable, the
build may embed a vendored bubblewrap binary and surface startup diagnostics
instead of stderr noise from helpers. Bubblewrap requires user namespaces; hosts
without them reject managed commands that would need that stack (for example some
WSL1 configurations).

### Windows

Elevated vs unelevated backends enforce **`WindowsExecutionRestrictionLevel`**
combined with legacy `read_only` / `workspace_write`-style semantics and richer
[`FileSystemAccessPolicy`] carve-outs when supported by the selected backend.

New `[permissions]` / split filesystem layouts are accepted only when the chosen
backend can enforce them—or when they remain equivalent to a legacy coarse
policy. Policies that would need unreadable carve-outs or reopened writable paths
under denied parents fail closed rather than weakening enforcement.

### All Platforms

The binary containing `agere-core` simulates the virtual `apply_patch` CLI when
`arg1` is `--agere-run-as-apply-patch`. See `agere-arg0` for dispatch details.

[`FileSystemAccessPolicy`]: ../protocol/src/permissions.rs

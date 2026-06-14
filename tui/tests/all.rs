// Single integration test binary that aggregates all test modules.
// The submodules live in `tests/suite/`.
mod test_backend;

#[allow(unused_imports)]
use agere_cli as _; // Keep dev-dep for cargo-shear; tests spawn the agere binary.

mod suite;

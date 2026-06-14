set positional-arguments

rust_min_stack := "8388608" # 8 MiB

# Display help
help:
    just -l

# `agere`
alias c := agere
agere *args:
    cargo run --bin agere -- "$@"

# `agere exec`
exec *args:
    cargo run --bin agere -- exec "$@"

# Start `agere exec-server` and run agere-tui.
[no-cd]
tui-with-exec-server *args:
    cargo build -p agere-cli --bin agere \
    && ./target/debug/agere exec-server --listen ws://127.0.0.1:0 & \
    exec_server_pid=$! \
    && trap "kill $$exec_server_pid 2>/dev/null" EXIT \
    && cargo run --bin agere -c exec_server_url=ws://127.0.0.1:0 "$@"

# Run the CLI version of the file-search crate.
file-search *args:
    cargo run --bin agere-file-search -- "$@"

# Build the CLI and run the app-server test client
app-server-test-client *args:
    cargo build -p agere-cli
    cargo run -p agere-app-server-test-client -- --agere-bin ./target/debug/agere "$@"

# format code
fmt:
    cargo fmt -- --config imports_granularity=Item 2>/dev/null

fix *args:
    cargo clippy --fix --tests --allow-dirty "$@"

clippy *args:
    cargo clippy --tests "$@"

install:
    rustup show active-toolchain
    cargo fetch

# Run `cargo nextest` since it's faster than `cargo test`, though including
# --no-fail-fast is important to ensure all tests are run.
#
# Run `cargo install cargo-nextest` if you don't have it installed.
# Prefer this for routine local runs. Workspace crate features are banned, so
# there should be no need to add `--all-features`.
test:
    RUST_MIN_STACK={{ rust_min_stack }} cargo nextest run --no-fail-fast

# Run the MCP server
mcp-server-run *args:
    cargo run -p agere-mcp-server -- "$@"

# Regenerate the json schema for config.toml from the current config types.
write-config-schema:
    cargo run -p agere-core --bin agere-write-config-schema

# Regenerate vendored app-server protocol schema artifacts.
write-app-server-schema *args:
    cargo run -p agere-app-server-protocol --bin write_schema_fixtures -- "$@"

[no-cd]
write-hooks-schema:
    cargo run -p agere-hooks --bin write_hooks_schema_fixtures

# Tail logs from the state SQLite database
log *args:
    if [ "${1:-}" = "--" ]; then shift; fi; cargo run -p agere-state --bin logs_client -- "$@"

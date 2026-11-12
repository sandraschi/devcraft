default:
    @just --list

# Run headless smoke test
smoke:
    cargo run -p devcraft-app -- --smoke

# Run desktop studio
run:
    cargo run -p devcraft-app

# Run MCP stdio server
mcp:
    cargo run -p devcraft-mcp -- --mcp

# Build all workspace targets
build:
    cargo build --workspace

# Run tests
test:
    cargo test --workspace

# Check clippy lints
lint:
    cargo clippy --workspace --all-targets -- -D warnings

# Build and stage dist/ (plain exes, installer, launcher zip, SHA256SUMS). Nothing is published.
release-dry:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1

# Same, then tag, push the tag and upload to GitHub (clean, pushed trunk required).
release:
    powershell -NoProfile -ExecutionPolicy Bypass -File scripts/release.ps1 -Publish

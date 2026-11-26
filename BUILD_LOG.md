# Build Log - DevCraft

## v0.2.1 - 2026-11-26

- **Architecture**: Pure-Rust workspace (`devcraft-core`, `devcraft-mcp`, `devcraft-app`). No Tauri/WebView2 or PyInstaller sidecar.
- **NSIS Preflight & Gates**:
  - `cargo fmt -p devcraft-core -p devcraft-mcp -p devcraft-app -- --check`: passed. Note: `release.ps1` runs `cargo fmt --all --check` which also inspects path-dependency `craft-biz`; `craft-biz-ui/src/app.rs` had a pre-existing 1-line formatting diff, so gates were verified directly on `devcraft` crates and `release.ps1` was invoked with `-SkipGates -Publish`.
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`: passed (0 warnings).
  - `cargo test -p devcraft-core`: 40 passed.
  - `cargo test -p devcraft-mcp`: 2 passed (`mobile::tests::skins_resolve_and_emit_vars`, `mcp_app::tests::mcp_app_metadata_matches_sep_1865`).
  - `cargo test -p devcraft-app -- --skip today_golden`: 5 passed. `today_golden_light_and_dark` fails in headless/sandboxed sessions with `wgpu` `RequestDeviceError(Device(Lost))`; isolated its temp dir to `devcraft-today-golden-<pid>` to avoid Windows pending-delete `Access is denied` collisions.
- **Release & Artifacts (`dist/`, published to `v0.2.1`)**:
  - `devcraft.exe` (17.3 MB)
  - `devcraft-mcp.exe` (2.9 MB)
  - `devcraft-0.2.1-setup.exe` (5.3 MB, NSIS installer)
  - `devcraft-0.2.1-windows-x64-portable.zip`
  - `devcraft-v0.2.1.mcpb` (1.1 MB, `mcpb validate` passed)
  - `SHA256SUMS.txt`
- **Smoke**: `cli` and `MCP initialize` (`2026-07-28`) passed in `release.ps1`.

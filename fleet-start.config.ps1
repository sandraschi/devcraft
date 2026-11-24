# Per-repo fleet start config for devcraft
# Assigned ports in mcp-central-docs/operations/WEBAPP_PORTS.md
# NOTE (2026-11-24): devcraft is a Rust egui desktop + stdio MCP. The only
# listener is the opt-in serve companion on 11259 (no frontend on 11258 -
# that port is held by tailscaled, unrelated). FrontendPort 0 keeps the
# health probe from flagging a half-stack.
@{
    Name         = 'devcraft'
    FrontendPort = 0
    BackendPort  = 11259
    HealthPath   = '/api/health'
    Backend = @{
        Kind       = 'custom'
        Command    = 'cargo run -p devcraft-mcp -- --mcp'
        WorkingDir = '.'
    }
    Frontend = @{
        Kind       = 'custom'
        Command    = 'cargo run -p devcraft-app'
        WorkingDir = '.'
    }
}

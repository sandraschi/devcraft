# Per-repo fleet start config for devcraft
# Assigned ports in mcp-central-docs/operations/WEBAPP_PORTS.md
@{
    Name         = 'devcraft'
    FrontendPort = 11258
    BackendPort  = 11259
    HealthPath   = '/health'
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

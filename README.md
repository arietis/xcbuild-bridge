# xcbuild-bridge

Minimal MCP server for Xcode simulator builds.

## Build

```
cargo build
```

## Test

```
cargo test
```

## Run (stdio MCP)

```
cargo run -q
```

## Codex CLI config example

```toml
[mcp_servers.xcbuild-bridge]
command = "/absolute/path/to/xcbuild-bridge"
args = []
```

## Core tools

- `session-set-defaults`
- `session-show-defaults`
- `session-clear-defaults`
- `list_sims`
- `discover_projs`
- `list_schemes`
- `get_sim_app_path`
- `install_app_sim`
- `launch_app_sim`
- `boot_sim`
- `erase_sims`
- `build_sim`
- `build_for_testing_sim`
- `test_without_building_sim`
- `discover_xctestrun`
- `smoke_sim`
- `test_sim`

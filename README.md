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
- `build_sim`
- `test_sim`

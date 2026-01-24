# xcbuild-bridge

Minimal JSON-over-stdin/stdout wrapper for `xcodebuild`.

## Build

```
cargo build
```

## Test

```
cargo test
```

## Usage

Input JSON:

```
{"args":["-version"]}
```

Run:

```
echo '{"args":["-version"]}' | cargo run -q
```

Output JSON:

```
{"ok":true,"exit_code":0,"terminated_by_signal":false,"stdout":"...","stderr":"","error":null}
```

## Request schema

- `args`: required array of strings (passed to `xcodebuild`)
- `cwd`: optional working directory

## Response schema

- `ok`: boolean
- `exit_code`: integer (or -1 on failure)
- `terminated_by_signal`: boolean
- `stdout`: string
- `stderr`: string
- `error`: optional string

---
name: octatree-testing
description: How OctaTree is tested (TOML-driven behavior tests with an in-memory tree, docs example checks, the demo project with Rojo) and how to benchmark. Use when writing or changing tests, or verifying a change end to end.
---

# Testing OctaTree

## Layers
1. **Unit tests** inline in `lib/*.rs` for pure code: `naming` (case styles, prefixes), `import` (old config parsing), `migrate` (TOML output is valid settings).
2. **Behavior tests** in `tests/modern.rs`: each test builds an `octatree.toml` as a string and a file tree as a list of paths (`MemReader::from_paths`, a trailing `/` declares an empty directory), calls `generate`, and asserts on the JSON. Use neutral names (`gameplay`, `ui`, ...). Top-level TOML keys must come before any `[table]`, so helpers like `SUFFIX` are inline `suffix_rules = [{ ... }]` keys placed right after `BASE`.
3. **Docs examples** in `tests/docs.rs`: every ```toml block in `README.md` and `docs/` must parse as settings (a block containing `template_file` is skipped). Write doc examples as complete, valid snippets.
4. **Disk reader**: `FsReader` behavior (file-as-directory, missing folder) is tested with a temp directory.

Every new option needs a default test and a non-default test, plus a validation-error test when it can be misconfigured.

## End to end with Rojo
`examples/basic` is a small Roblox project with its own `rokit.toml`. After engine changes:
```powershell
cargo run --release -- generate --root examples/basic --strict
cargo run --release -- generate --root examples/basic --check --strict
cd examples/basic; rojo build default.project.json --output $env:TEMP\basic.rbxl
```
Rojo must accept the generated file. (Run `rojo` from a folder with a `rokit.toml`, or Rokit can't find the tool.)

## Gates
```powershell
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```
CI runs the same on Linux and Windows. Paths in tests must be posix (`/`) strings; the output must not depend on the OS.

## Performance
Don't claim speedups without numbers. To measure, generate a synthetic project (thousands of files) in a temp folder and time `octatree generate --dry-run` with `hyperfine`; profile before adding `rayon`.

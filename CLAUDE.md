# OctaTree

OctaTree is a Rust CLI that scans a Roblox/Luau project's `src/` folder and generates the Rojo project file (`default.project.json`). A TOML config (`octatree.toml`) decides which folders are scanned and where every file is mounted (any number of named containers such as shared, server, client).

## Principle: empty by default
OctaTree must not assume any project structure. The defaults are: project name `Template`, no modules, no suffix rules, no naming exceptions, containers `shared` (`ReplicatedStorage`) and `server` (`ServerScriptService`), and a default tree that is only `ReplicatedStorage` and `ServerScriptService`, each with `$ignoreUnknownInstances`. `octatree init` writes a starter config that describes no structure. Never add defaults, examples or fixtures taken from a particular game (folder names, bootstrap paths, naming conventions). Use neutral names in tests and docs (`gameplay`, `ui`, ...).

## Code map (`lib/`)
The Rust sources live in `lib/` (not `src/`, to avoid clashing with the `src/` folder Rojo projects use); `Cargo.toml` points `[lib]`/`[[bin]]` at it. `src/...` paths in configs, rules, tests and docs always refer to the *user's game project*, never to this repo.
- `settings.rs`: `octatree.toml` schema (serde, `deny_unknown_fields`), validation, and the pure routing decisions (`direct`, `decide`, `node_name`); also `STARTER_TOML`.
- `engine.rs`: walks the modules, applies rules, mounts into containers, applies `[nodes]`/`[rojo]`; default tree; service/class inference.
- `naming.rs`: `NamingRules` (pascal/camel/snake/keep, `keep_prefixes`, word-boundary vs starts_with, short words).
- `scan.rs`: `DirReader` trait, `FsReader` (all disk reads), `MemReader` (tests), deterministic entry order.
- `watch.rs`: `octatree watch`/`stop`: `Watcher` (polls: re-reads config, regenerates in memory, writes only on change) and `Control` (heartbeat + stop-marker files in the OS temp dir, keyed by a stable hash of the canonical root).
- `explain.rs`: `octatree explain`. `emit.rs`: write-if-changed. `error.rs`.
- `import.rs` + `migrate.rs`: read an old JSON routing config (import-only) and convert it to TOML.
- `main.rs`: CLI. `lib.rs`: `generate(settings, reader)`.
CLI: `octatree generate|watch|stop|explain|migrate <path>|init`. Config: only `<root>/octatree.toml` (or `--config x.toml`).

## Tests
Unit tests inline; `tests/modern.rs` (behavior via TOML text + `MemReader`, validation, explain, import); `tests/watch.rs` (control files incl. stale heartbeat, watcher on a temp project); `tests/docs.rs` parses every ```toml block in `README.md` and `docs/`, so doc examples must stay valid. `examples/basic` is a demo Roblox project (own `rokit.toml` with Rojo): `octatree generate --root examples/basic`; it is never embedded in the binary or the release zips.

## Commands (PowerShell, Windows is the primary dev OS)
```powershell
cargo build
cargo test
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo run -- generate --root examples/basic --dry-run
cargo run -- explain --root examples/basic src/gameplay/combat/CombatServiceClient.luau
```
A change is not done until fmt, clippy (warnings denied) and tests pass. Report failures as they are.

## Working rules
- Keep logic in the library crate; `main.rs` stays thin. Separate I/O (`scan`, `emit`) from pure logic (`settings`, `naming`).
- Output must be deterministic: sorted entries, insertion-ordered JSON, posix `/` paths, `\n` newlines.
- Ask before adding dependencies beyond those listed in `octatree-rust-style`.
- Don't claim performance gains without a measurement.
- When you change the schema, CLI or semantics: update `docs/`, `README.md`, `STARTER_TOML` and the `octatree-config-design` skill, and add tests.
- Never push, tag or create releases unless the user asks. Branch is `master`.
- Treat the user's project data (paths, names) as private; don't send it to external services, and keep it out of published files.

## Decisions (made with the user)
- Config format is **TOML** (snake_case keys, unknown keys are errors).
- **Most specific rule wins.** Per node: a rule matching the node's own path (more literal path segments, then later in the file, wins) > suffix rule > target inherited from the nearest ancestor directory rule > `default_to`.
- `init` folders (`init.luau`, `init.server.luau`, `init.client.luau`) are mounted as one node and follow the same rules.
- `keep_prefixes` match at a **word boundary** by default (`prefix_match = "starts_with"` for plain prefix).
- Defaults are empty (see Principle). Suffix rules are opt-in.
- There is no engine for the old JSON config format; `octatree migrate <path>` imports it (rules for server-only / not-on-server / both, prefixes, plus an explicit `server`/`service` suffix rule, `modules = []`) and the developer fills in their folders. `generate` rejects non-TOML configs with a hint.
- `$className` support: rule `class_name`; `[nodes."<tree path>"]` (`class_name`, `properties`, `path`, `ignore_unknown_instances`) applied before mounting; `[rojo]` copies extra top-level project fields after `name` (`name`/`tree` forbidden).
- Mount creation: a missing depth-0 segment that is a known service (`SERVICES` in `engine.rs`) and a depth-1 `StarterPlayerScripts`/`StarterCharacterScripts` get `$className` = their name (also for a final mount segment); any other final segment is a `Folder`; other intermediate nodes get no class. Existing nodes are never modified by mounting. Validated with Rojo 7.7.1 (`rojo build`) on `examples/basic`.
- **Watch/stop** (user design): `watch` runs in the foreground and prints that `octatree stop` can be run from another terminal (Ctrl+C is not the only way, because many people use it to copy). No daemon, PID or signals: the watcher refreshes a heartbeat file (stale after 5 s) and `stop` writes a stop marker the watcher polls for; one watcher per project. Change detection is polling (default 500 ms, min 100) by regenerating in memory and comparing, so no `notify` dependency; invalid config edits are reported once and the last good config is kept.
- `extensions` are anchored name endings (default `[".luau"]`); longest wins.
- Entry order: case-insensitive ordinal, tie-break byte-wise (deterministic on every OS).
- Rules that never match, missing module folders and name collisions produce warnings (`on_collision`: `warn`/`error`/`last-wins`); non-UTF-8 names are skipped with a warning.
- Reading a *file* path as a directory counts as "not a directory" (Windows error 267).

## Releasing
Pushing a tag `vX.Y.Z` (must equal `Cargo.toml`'s version) runs `.github/workflows/release.yml`: fmt + clippy + tests, then builds windows-x86_64, linux-x86_64, linux-aarch64, macos-aarch64, macos-x86_64 and publishes `octatree-<version>-<os>-<arch>.zip` (binary + LICENSE + README) plus `checksums.txt`. Names follow Rokit's asset detection, so users install with `rokit add danieluntask/octatree`. Repo: https://github.com/DanielUnTask/OctaTree (MIT). Steps are in `docs/releasing.md`. The workflows have never run: expect to fix small CI issues on the first tag. `.github/workflows/ci.yml` runs fmt/clippy/test on Linux and Windows for pushes to `master` and PRs. `/legacy/` in `.gitignore` is local-only scratch space and must stay ignored.

## Not done yet
Benchmarks / performance work (`rayon` only if measured; polling cost on very large trees is unmeasured), `--json` diagnostics.

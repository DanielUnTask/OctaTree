---
name: octatree-rust-style
description: Rust conventions, architecture, performance guidelines and dependency policy for OctaTree. Use when writing, structuring, or reviewing any Rust code in this repo, adding a crate, or optimizing the generator.
---

# OctaTree Rust conventions

## Toolchain & gates
Edition 2024, stable toolchain. Before calling work done run, in order:
```powershell
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```
Report failures verbatim; never claim "done" without running them. Prefer `cargo check` while iterating.

## Architecture (library + thin binary)
Keep logic in a library crate so it is testable and reusable; `main.rs` only parses args and prints.

```
lib/             # Rust sources (not `src/`: that name belongs to the user's Rojo project)
  main.rs        # CLI entry (clap), exit codes, printing
  lib.rs         # re-exports, `generate()` entry point
  settings.rs    # octatree.toml schema, validation, pure routing decisions
  naming.rs      # NamingRules (pure)
  scan.rs        # DirReader trait; FsReader (all disk reads), MemReader (tests)
  engine.rs      # walker + container mounting + default tree
  import.rs / migrate.rs  # old JSON config -> octatree.toml
  explain.rs / emit.rs / error.rs
```
(Current layout; see `CLAUDE.md` code map.)
Separate **scan (I/O)** → **route (pure)** → **build tree (pure)** → **emit (I/O)**. Pure stages take plain data so tests need no disk.

## Performance (a goal; measure before claiming anything)
- One directory walk total; never re-`exists`/re-stat. Use `DirEntry::file_type()` (no extra syscall) instead of `metadata()`.
- Normalize each path to a posix `String`/`Cow<str>` once; build it incrementally (push segment, truncate on return) instead of re-joining.
- Compile all rule matchers once at startup (`GlobSet`, `HashSet<Box<str>>`), O(1)/O(log n) lookups per entry; no per-entry regex compilation.
- Avoid `clone()` of subtrees; build nodes by value and move them. Pre-size `Vec`/maps when counts are known.
- Optional: parallelize per top-level module with `rayon`, then merge in **configured order** so output stays deterministic. Gate on benchmarks, not intuition (`criterion` or `hyperfine` on a large synthetic `src/`).
- Write output only if bytes differ (preserves mtime so Rojo/watchers don't churn). Use a buffered write of one pre-rendered `String`.
- Release profile: `lto = "thin"`, `codegen-units = 1`, `panic = "abort"` (only if no cleanup needed).

## Determinism
Output must be byte-identical across runs and OSes: sort directory entries (case-insensitive ordinal, then byte-wise), keep JSON objects insertion-ordered (`serde_json` with `preserve_order`), LF line endings, 2-space indent, no trailing newline.

## Error handling
- Library: typed errors with `thiserror`; no `unwrap`/`expect`/`panic!` outside tests (an `expect` with an invariant message is OK for truly impossible states).
- Binary: `anyhow`/`miette`-style reporting with the config path and offending key/path in the message. Non-zero exit on error; warnings go to stderr and don't fail unless `--strict`.
- Unreadable entry / non-UTF-8 name: report with the path, skip or fail per `--strict`; never silently drop.
- Config problems (unknown key, rule pointing at a non-existent path, ambiguous overlap) are **warnings with the rule name**, so users can fix typos like `src/feature/ui`.

## Dependency policy
Small and justified. In use: `serde`, `serde_json` (`preserve_order`), `clap` (derive), `thiserror`, `anyhow`, `globset`, `toml` (`preserve_order`). Ask the user before adding anything else (watchers like `notify`, async runtimes, etc.). Commit `Cargo.lock` (this is a binary).

## Style
- `&str`/`&Path` in parameters, return owned data; use `Cow` only where profiling shows allocation pressure.
- Prefer enums and bitmasks over pairs of booleans for routing (destinations are a `Mask` over the configured containers) and make illegal states unrepresentable.
- `#[derive(Debug, Clone, PartialEq, Eq)]` on data types; `#[serde(deny_unknown_fields)]` on config structs so typos fail loudly.
- Doc comments (`///`) on every public item; module docs on each file explaining its stage.
- Keep functions small and pure; no global state.
- Paths: use `std::path::Path` for filesystem, but **always** convert to posix `/` strings for rule matching and output; never use `\\` in generated JSON.
- Windows is the primary dev OS: test path normalization, case-insensitive FS assumptions, and long paths.

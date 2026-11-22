---
name: octatree-config-design
description: Reference for OctaTree's configuration (octatree.toml schema, defaults, rule semantics, containers, nodes, templates, naming, CLI). Use when changing the schema, rule semantics or CLI, or adding any customization option.
---

# OctaTree configuration (implemented in `lib/settings.rs`, `lib/engine.rs`)

The developer defines their project's structure in config; OctaTree assumes none. Format: **TOML**, snake_case keys, unknown keys are errors (`deny_unknown_fields`). Source of truth for the schema is `settings.rs` and `settings::STARTER_TOML` (what `octatree init` writes); keep them in sync with this file and with `docs/configuration.md`.

## Defaults (all empty)
Project name `Template`, no modules, no rules, no suffix rules, no naming exceptions, containers `shared` → `ReplicatedStorage` and `server` → `ServerScriptService`, `default_to = ["shared"]`. With the default two containers and no template, the tree is only `ReplicatedStorage` and `ServerScriptService`, each `{ "$ignoreUnknownInstances": true }`; with other containers it is a bare `DataModel`. Never add a default that encodes a particular game's layout or conventions.

## Schema (all keys optional)
| Key | Default | Meaning |
|---|---|---|
| `version` | 1 | Only `1` is accepted |
| `project_name` | `"Template"` | `name` in the generated file |
| `source_dir` | `"src"` | Folder scanned, project-relative |
| `output` | `"default.project.json"` | Default output path |
| `modules` | `[]` | Folders under `source_dir`, in output order (single folder names) |
| `extensions` | `[".luau"]` | Anchored name endings; longest wins; leading dot optional |
| `init_files` | `init.luau`, `init.server.luau`, `init.client.luau` | A folder containing one becomes ONE node `{ $path }` |
| `default_to` | `["shared"]` (or first container) | Where unmatched nodes go |
| `on_collision` | `"warn"` | `warn` / `error` / `last-wins` |
| `ignore` | `[]` | Globs, never scanned |
| `[containers.<name>]` `mount` | `shared`, `server` (see above) | Mount points; any number (max 64); order = declaration order; listing any replaces the defaults |
| `[[suffix_rules]]` `suffix`, `to`, `case_insensitive` | none | Suffix routing; first match wins |
| `[naming]` `case`, `keep_prefixes`, `prefix_match`, `short_word_max` | `pascal`, `[]`, `word`, `2` | `case`: pascal/camel/snake/keep; `prefix_match`: `word` or `starts_with`; words ≤ `short_word_max` become ALL CAPS (0 disables) |
| `[[rules]]` `match`, `to`, `rename`, `set`, `class_name`, `name` | none | See below; at least one of `to`/`rename`/`set`/`class_name` |
| `[nodes."<tree path>"]` `class_name`, `properties`, `path`, `ignore_unknown_instances` | none | Sets `$className`/`$properties`/`$path`/`$ignoreUnknownInstances` on nodes of the tree (creates them if missing), applied before mounting |
| `[rojo]` | empty | Extra top-level project fields (`globIgnorePaths`, `placeId`, ...) written after `name`; `name`/`tree` forbidden |
| `[template]` / `template_file` | default tree | Contents of `tree` (inline TOML table, or a JSON file relative to the config); not both; used as is |

## Rule semantics ("most specific wins")
Per **node** (file, or init folder):
1. Rules whose `match` glob hits the node's **own path**, layered least→most specific: specificity = number of literal (wildcard-free) path segments, ties → later rule wins. Later layers override `to`/`rename`/`class_name`, and `set` tables merge.
2. else a `suffix_rules` entry matching the node name (extension stripped; for init folders, the folder name);
3. else the target inherited from the **nearest ancestor directory** rule that has `to` (a deeper rule replaces the inherited one);
4. else `default_to`.
`to = []` excludes the node everywhere. `rename`, `set`, `class_name` are node-local (not inherited). Patterns: `globset` with `literal_separator` (`*` stays within a segment, `**` crosses), project-relative, forward slashes; no wildcards = exact path. A rule that never matches produces a warning.

## Mounting
Each container's module nodes are inserted at its `mount` path inside `tree`. Missing nodes: known services (depth 0) and `StarterPlayerScripts`/`StarterCharacterScripts` (depth 1) get `$className` = name (also as the final segment); other final segments become `Folder`; other intermediate nodes get no class. Existing nodes (template, `[nodes]`, default tree) are never modified by mounting.

## Importing an old config
`octatree migrate <path>` (`import.rs` + `migrate.rs`) reads an old JSON routing config (`context-server`, `server-black-list`, `duplicated-modules`, `dontUsePascalCase.prefixes`) and writes `octatree.toml`: duplicated → `to = ["shared","server"]`, not-on-server → `to = ["shared"]`, server-only → `to = ["server"]` (+ `rename` from `$name`), prefixes → `keep_prefixes` with `prefix_match = "starts_with"`, an explicit `server`/`service` suffix rule, `modules = []`. Rules are emitted in that order so "later wins" keeps the old precedence on equal paths. There is no engine for the old format: `generate` only reads TOML.

## CLI
`octatree generate [--root --config --out --dry-run --check --strict]` · `explain <path>` · `migrate <path> [--root --out --force]` · `init [--root --force]`. Exit codes: 0 ok, 1 `--check` out of date, 2 error.

## Adding an option
Add the field to `RawSettings` + `Settings` with an empty/neutral default → validate it in `from_toml` with an error naming the key → implement it in `engine.rs` (pure decisions go in `settings.rs`) → show it in `explain` if it affects routing → add a commented example to `STARTER_TOML` → test default and non-default in `tests/modern.rs` → update this table, `docs/configuration.md` and `CLAUDE.md`.

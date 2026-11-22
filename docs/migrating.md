# Importing an old config

If you already have a routing config in the JSON format of an older, similar tool (`tool-config.json`), `octatree migrate` converts it into an `octatree.toml`:

```powershell
octatree migrate path/to/tool-config.json
```

By default the result is written to `octatree.toml` in the current folder (`--root DIR` or `--out FILE` change that). An existing file is never overwritten unless you pass `--force`.

OctaTree itself only reads `octatree.toml`; the old JSON file is not used when generating.

## What gets converted

| Old key | New TOML |
|---|---|
| `context-server`: `{ "$path": P }` | `[[rules]] match = P, to = ["server"]` |
| `context-server`: `{ "$path": P, "$name": N }` | same, plus `rename = N` |
| `server-black-list`: `{ "$path": P }` | `[[rules]] match = P, to = ["shared"]` |
| `duplicated-modules`: `"name"` | `[[rules]] match = "src/name", to = ["shared", "server"]` |
| `duplicated-modules`: `{ "$path": P }` | same with `match = P` |
| `dontUsePascalCase.prefixes` | `[naming] keep_prefixes`, with `prefix_match = "starts_with"` to keep the old plain prefix matching |

The old tool also sent names ending in `server` or `service` to the server, so the converted file contains that as an explicit `[[suffix_rules]]` entry. Delete it if your project doesn't use that convention.

Keys the importer doesn't know are listed as warnings and not converted.

## What you do afterwards

The converted file has `modules = []`: the old format has no list of folders, so **you list your own** (the folders under `src` that should be scanned). It also has no project name (`Template`) until you set `project_name`.

1. Edit `octatree.toml`: set `modules` and `project_name`.
2. Check the routing of a few files with `octatree explain <path>`.
3. Run `octatree generate`.

## Where results can differ from the old tool

The new engine is not a clone of the old one. Known differences:

- With several overlapping rules, the **deepest folder rule wins**. The old tool let a "not on the server" folder beat a "both" rule in a sub-folder below it; now the sub-folder rule wins.
- Folders with an `init` file follow the same rules as every other node.
- Names are only stripped of their real file ending.
- The old tool added a fixed set of folders and paths to the tree. OctaTree starts with only `ReplicatedStorage` and `ServerScriptService`; add anything else with [`[nodes]`](configuration.md#node-settings-nodes-class-names-properties-paths) or a [template](configuration.md#template).

# Command line

```
octatree <COMMAND> [OPTIONS]
```

| Command | Purpose |
|---|---|
| [`generate`](#generate) | Write the Rojo project file |
| [`watch`](#watch) | Keep the project file up to date while you work |
| [`stop`](#stop) | Stop a running `watch` from another terminal |
| [`explain`](#explain) | Show why a path is routed the way it is |
| [`migrate`](#migrate) | Convert an old JSON config into `octatree.toml` |
| [`init`](#init) | Write a commented starter `octatree.toml` |

## Finding the config

`generate` and `explain` read `<root>/octatree.toml`, where the root is `--root` (default: the current folder). `--config <file>` points at another `.toml` file. Other file types are rejected with a hint to use `migrate`.

## generate

```powershell
octatree generate [--root DIR] [--config FILE] [--out FILE] [--dry-run] [--check] [--strict]
```

| Option | Description |
|---|---|
| `--root DIR` | Project root. Default `.` |
| `--config FILE` | Config file to use |
| `--out FILE` | Output file. Default: the config's `output` (normally `<root>/default.project.json`) |
| `--dry-run` | Print the JSON to stdout and write nothing. Warnings still go to stderr |
| `--check` | Write nothing; exit 1 if the output file is missing or differs from what would be generated |
| `--strict` | Treat any warning as an error |

The output file is only rewritten when its content changes.

Typical CI step:

```powershell
octatree generate --check --strict
```

## watch

```powershell
octatree watch [--root DIR] [--config FILE] [--out FILE] [--interval-ms N]
```

Generates once, then regenerates whenever the result changes: when you add, remove or rename files and folders, or edit `octatree.toml`. Run it in a terminal next to `rojo serve`.

```
Watching . (checking every 500 ms), writing .\default.project.json
To stop it, run `octatree stop` in another terminal (Ctrl+C also works).
generated .\default.project.json
```

| Option | Description |
|---|---|
| `--root DIR` | Project root. Default `.` |
| `--config FILE` | Config file to use |
| `--out FILE` | Output file. Default: the config's `output` |
| `--interval-ms N` | How often to check, in milliseconds. Default `500`, minimum `100` |

How it behaves:

- The file is only written when its content changes, so Rojo isn't triggered for nothing.
- A change to `octatree.toml` is reloaded automatically. If you save an invalid config, the error is shown once and the watcher keeps working with the last good configuration; fixing the file recovers it.
- Errors and warnings are shown when they appear (not on every check) and never stop the watcher.
- Only one watcher can run per project; starting a second one tells you to run `octatree stop`.
- It checks by re-reading the project at the chosen interval instead of listening to file-system events, which keeps it dependable on network drives and WSL. Very large projects can raise `--interval-ms`.

## stop

```powershell
octatree stop [--root DIR]
```

Stops the `octatree watch` that is running for the project, from any other terminal. It uses a small heartbeat file in your temp folder (nothing is written into your project): the watcher refreshes it every second, so a watcher that was killed or crashed is recognized as gone after about 5 seconds. If no watcher is running, `stop` just says so.

## explain

```powershell
octatree explain <PATH> [--root DIR] [--config FILE]
```

`<PATH>` is relative to the project root, for example `src/gameplay/ui/Hud.luau`. It also works for folders. The output lists, for every level of the path, the rules that apply and, at the end, the node name and the containers the file goes to, together with the reason (a rule, a suffix rule, an inherited folder rule, or `default_to`).

It tells you when:

- the path is ignored (`ignore`), or is not in `modules`;
- the file isn't a source file (extension not listed);
- a folder with an `init` file swallows everything inside it.

## migrate

```powershell
octatree migrate <PATH> [--root DIR] [--out FILE] [--force]
```

Converts an old JSON routing config into an `octatree.toml`. `<PATH>` is the old file, wherever it is. See [Importing an old config](migrating.md).

| Option | Description |
|---|---|
| `--root DIR` | Where `octatree.toml` is written. Default `.` |
| `--out FILE` | Exact output file. Default `<root>/octatree.toml` |
| `--force` | Overwrite an existing file (without it, an existing file is never touched) |

## init

```powershell
octatree init [--root DIR] [--force]
```

Writes a commented starter `octatree.toml` into the project root. It describes no structure: nothing is scanned until you list `modules`. Refuses to overwrite an existing file unless `--force` is given.

## Exit codes

| Code | Meaning |
|---|---|
| `0` | Success |
| `1` | `generate --check`: the output file is missing or out of date |
| `2` | Error (invalid config, unreadable folder, collision with `on_collision = "error"`, warnings under `--strict`, ...) |

## Output streams

The generated JSON goes to the output file (or to stdout with `--dry-run`). Status messages and warnings go to stderr, so `octatree generate --dry-run > tree.json` captures clean JSON.

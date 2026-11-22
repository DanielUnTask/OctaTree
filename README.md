# OctaTree

OctaTree generates the Rojo project file (`default.project.json`) for a Luau game by scanning your source folders. You describe in a small `octatree.toml` which folders to scan and where each file should be mounted (shared, server, or any container you define); OctaTree produces the same tree every time, quickly.

It starts empty: a new project gets only `ReplicatedStorage` and `ServerScriptService`, each with `$ignoreUnknownInstances`. Your folder structure, naming and routing are yours to define.

## Install

With [Rokit](https://github.com/rojo-rbx/rokit) (recommended), from your project folder:

```powershell
rokit add danieluntask/octatree
rokit install
```

Or download the zip for your platform from the [releases page](https://github.com/DanielUnTask/OctaTree/releases) and put `octatree` somewhere on your `PATH`. To build it yourself (Rust 1.88 or newer):

```powershell
cargo install --git https://github.com/DanielUnTask/OctaTree
```

## Quick start

```powershell
octatree init                # writes an empty, commented octatree.toml
```

List the folders under `src` that OctaTree should scan:

```toml
modules = ["gameplay", "ui"]
```

```powershell
octatree generate            # writes default.project.json
```

While you work, keep the file in sync automatically (in a second terminal next to `rojo serve`):

```powershell
octatree watch               # regenerates when files or the config change
octatree stop                # run from another terminal to stop it
```

A small demo project lives in [`examples/basic`](examples/basic): `octatree generate --root examples/basic`.

## What you can configure in `octatree.toml`

Everything below is optional and empty by default.

```toml
project_name = "MyGame"
modules      = ["gameplay", "ui"]               # folders under src to scan
ignore       = ["**/*.spec.luau"]               # never scanned
on_collision = "warn"                           # warn | error | last-wins
default_to   = ["shared"]                       # where unmatched files go

[containers.shared]                             # named mount points (as many as you like)
mount = "ReplicatedStorage"
[containers.server]
mount = "ServerScriptService"
[containers.client]
mount = "StarterPlayer/StarterPlayerScripts"

[[suffix_rules]]                                # route by file name ending
suffix = ["server"]
to = ["server"]

[naming]
case = "pascal"                                 # pascal | camel | snake | keep
keep_prefixes = ["use"]                         # names that keep their casing
short_word_max = 2                              # ui -> UI

[[rules]]                                       # route, rename or tag paths (globs)
match = "src/gameplay/admin"
to = ["server"]

[[rules]]
match = "src/ui/shared_widgets"
to = ["shared", "client"]                       # mount in several containers
rename = "Widgets"

[[rules]]
match = "**/*.client.luau"
set = { RunContext = "Client" }                 # becomes $properties

[nodes.HttpService]                             # service nodes with $className/$properties
class_name = "HttpService"
properties = { HttpEnabled = true }

[rojo]                                          # extra top-level project fields
globIgnorePaths = ["**/*.spec.lua"]
```

Features:

- **Rules with globs.** Route, rename, exclude (`to = []`), set a `class_name` or add `$properties`; folder rules apply to everything below, and the most specific rule wins.
- **Any number of containers**, each mounted wherever you want in the tree.
- **Extra folders.** Mount packages, bootstrap scripts or any file or folder at a place and under a name you choose (`[nodes."ReplicatedStorage/Packages"]` with `path = "Packages"`); see [docs/extra-folders.md](docs/extra-folders.md).
- **`$className` where you need it.** `[nodes]` sets class, properties, `$path` and `$ignoreUnknownInstances` on any node of the tree; `[rojo]` adds top-level fields such as `globIgnorePaths`. Known services and `StarterPlayerScripts` get their class automatically when OctaTree creates them.
- **Suffix routing**, **naming styles**, **ignore patterns** and a **collision policy**.
- **Custom tree template** (inline TOML or a JSON file) instead of the default two-service tree.
- **Helpful diagnostics.** Typos in the config are errors with line numbers, rules that match nothing produce a warning, and `octatree explain <path>` shows which rule put a file where it went.
- **Watch mode.** `octatree watch` regenerates when files or the config change (a broken config edit never stops it), and `octatree stop` ends it from another terminal.
- **Safe to automate.** Output is deterministic, the file is only rewritten when it changes, and `octatree generate --check` fails when it is out of date.
- **Import.** `octatree migrate path/to/old-config.json` converts an old JSON routing config into `octatree.toml`.

## Documentation

- [Getting started](docs/getting-started.md): install, first config, how routing works
- [Configuration reference](docs/configuration.md): every `octatree.toml` key, rule semantics, recipes
- [Command line](docs/cli.md): commands, flags, exit codes
- [Extra folders](docs/extra-folders.md): mount packages, bootstrap scripts and other folders outside your modules, under any name
- [Importing an old config](docs/migrating.md)
- [Releasing](docs/releasing.md) (maintainers): tags, release assets, Rokit install

## Development

```powershell
cargo fmt --all
cargo clippy --all-targets -- -D warnings
cargo test
```

The Rust sources are in `lib/` (so the name `src` stays free for Rojo projects).

## License

MIT, see [LICENSE](LICENSE).

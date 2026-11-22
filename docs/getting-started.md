# Getting started

OctaTree scans your game's source folder and writes the Rojo project file (`default.project.json`) for you. You describe *which folders to scan* and *where each file should be mounted* in a small `octatree.toml`; OctaTree does the rest, the same way every time.

- [Configuration reference](configuration.md): every key in `octatree.toml`
- [Command line](cli.md): all commands and flags
- [Importing an old config](migrating.md)

## 1. Install

**With Rokit** (recommended). From your project folder:

```powershell
rokit add danieluntask/octatree
rokit install
```

**From a release.** Download the zip for your platform from the [releases page](https://github.com/DanielUnTask/OctaTree/releases), extract `octatree` (`octatree.exe` on Windows) and put it on your `PATH`.

**From source.** You need Rust 1.88 or newer:

```powershell
cargo install --git https://github.com/DanielUnTask/OctaTree
```

or, inside a clone of the repository, `cargo build --release` (the binary is `target/release/octatree`).

Check it works with `octatree --version`.

## 2. Create a config

Run this in your project's root folder (the one that contains `src`):

```powershell
octatree init
```

It writes a commented `octatree.toml` that describes **no structure at all**. If you generate right away you get the minimal tree:

```json
{
  "name": "Template",
  "tree": {
    "$className": "DataModel",
    "ReplicatedStorage": { "$ignoreUnknownInstances": true },
    "ServerScriptService": { "$ignoreUnknownInstances": true }
  }
}
```

Everything else is up to you.

## 3. Say what to scan

List the folders under `src` that OctaTree should scan, and name your project:

```toml
project_name = "MyGame"
modules = ["gameplay", "ui"]
```

Every `.luau` file inside a module becomes a node, and folders become `Folder` nodes. A folder that contains an `init.luau` (or `init.server.luau` / `init.client.luau`) is mounted as a single node instead.

## 4. Generate

```powershell
octatree generate
```

OctaTree writes `default.project.json` next to your config. If the file would be identical it is left untouched (so file watchers don't fire needlessly).

```powershell
octatree generate --dry-run     # print the JSON, write nothing
octatree generate --check       # exit 1 if the file is missing or out of date (good for CI)
octatree generate --strict      # treat warnings as errors
```

### Keeping it up to date

`default.project.json` lists every file, so after you add, remove or rename files you need to generate again (changing the contents of an existing file doesn't). Instead of running `generate` by hand, leave this running in a second terminal:

```powershell
octatree watch
```

It prints a line each time it regenerates, reloads `octatree.toml` when you edit it, and shows errors without exiting. Stop it with `octatree stop` from another terminal (Ctrl+C also works). See [`watch`](cli.md#watch).

## 5. How files are routed

Every file ends up in one or more **containers**. By default there are two:

| Container | Mounted at |
|---|---|
| `shared` | `ReplicatedStorage` |
| `server` | `ServerScriptService` |

For each file, OctaTree picks the destination with this order of priority (first match wins):

1. a **rule** in `octatree.toml` that matches the file's own path;
2. a **suffix rule** (none by default; you add them, see below);
3. the destination **inherited from the nearest parent folder** that has a rule;
4. `default_to` (by default `shared`).

Example. With this config:

```toml
project_name = "MyGame"
modules = ["gameplay", "ui"]

[[suffix_rules]]
suffix = ["server"]
to = ["server"]

[[rules]]
match = "src/gameplay/admin"
to = ["server"]
```

and these files:

```
src/gameplay/Inventory.luau
src/gameplay/MatchServer.luau
src/gameplay/admin/Tools.luau
src/ui/Hud.luau
```

`octatree generate` produces:

```json
{
  "name": "MyGame",
  "tree": {
    "$className": "DataModel",
    "ReplicatedStorage": {
      "$ignoreUnknownInstances": true,
      "Gameplay": {
        "$className": "Folder",
        "Inventory": { "$path": "src/gameplay/Inventory.luau" }
      },
      "UI": {
        "$className": "Folder",
        "Hud": { "$path": "src/ui/Hud.luau" }
      }
    },
    "ServerScriptService": {
      "$ignoreUnknownInstances": true,
      "Gameplay": {
        "$className": "Folder",
        "Admin": {
          "$className": "Folder",
          "Tools": { "$path": "src/gameplay/admin/Tools.luau" }
        },
        "MatchServer": { "$path": "src/gameplay/MatchServer.luau" }
      }
    }
  }
}
```

| File | Goes to | Why |
|---|---|---|
| `Inventory.luau` | shared | `default_to` |
| `MatchServer.luau` | server | its name ends in `server` (suffix rule) |
| `admin/Tools.luau` | server | inherited from the `admin` folder rule |
| `ui/Hud.luau` | shared | `default_to` |

Folders appear only in the containers where something is mounted: `Gameplay` exists in both places here, each holding only its own files. Node names are PascalCase by default (`ui` becomes `UI`); see [naming](configuration.md#naming).

## 6. Find out why a file went where it did

```powershell
octatree explain src/gameplay/MatchServer.luau
```

```
src/gameplay/MatchServer.luau
  src/gameplay
  src/gameplay/MatchServer.luau
    name: MatchServer
    => [server] via suffix rule #0 (server)
```

`explain` lists every rule that applies at each level of the path and the one that decided the result. Use it whenever a file lands somewhere unexpected.

## Troubleshooting

| Symptom | Cause / fix |
|---|---|
| The output only has the two services | `modules` is empty or misspelled. Module names are folder names under `src`. |
| `warning: rules[N] ... matched nothing` | The rule's `match` path doesn't exist or has a typo. Paths are project-relative with forward slashes, e.g. `src/gameplay/admin`. |
| `warning: module folder ... does not exist` | A name in `modules` has no matching folder. |
| `warning: name collision` | Two files map to the same node name (`foo_bar.luau` and `FooBar.luau`). Rename one, or use a `rename` rule. |
| `unknown field ...` error | A key in `octatree.toml` is misspelled. The message shows the line and lists the valid keys. |
| A file isn't in the output | It doesn't end in a configured `extensions` entry, or matches an `ignore` glob. `octatree explain <path>` says which. |
| `octatree.toml not found` | Run `octatree init`, or pass `--config`. |

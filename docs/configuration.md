# Configuration reference (`octatree.toml`)

All keys are optional, and the defaults describe no structure: nothing is scanned or routed until you say so. Unknown keys are **errors** (with the line number), so typos never go unnoticed. Paths in the config are relative to the project root and always use forward slashes (`src/gameplay/ui`).

Generate a commented starter with `octatree init`.

## Overview

```toml
version = 1
project_name = "MyGame"
source_dir = "src"
output = "default.project.json"
modules = ["gameplay", "ui"]
extensions = [".luau"]
init_files = ["init.luau", "init.server.luau", "init.client.luau"]
default_to = ["shared"]
on_collision = "warn"
ignore = ["**/*.spec.luau"]

[containers.shared]
mount = "ReplicatedStorage"

[containers.server]
mount = "ServerScriptService"

[[suffix_rules]]
suffix = ["server"]
to = ["server"]

[naming]
case = "pascal"
keep_prefixes = ["use"]
prefix_match = "word"
short_word_max = 2

[[rules]]
match = "src/gameplay/admin"
to = ["server"]
```

## General keys

| Key | Default | Description |
|---|---|---|
| `version` | `1` | Config format version. Only `1` is accepted. |
| `project_name` | `"Template"` | The `name` written into the generated project file. |
| `source_dir` | `"src"` | Folder that contains your modules. |
| `output` | `"default.project.json"` | Output file, relative to the project root. `--out` overrides it. |
| `modules` | `[]` | Folders inside `source_dir` to scan, in output order. Each must be a single folder name (no `/`). **Empty by default: nothing is scanned until you list modules.** |
| `extensions` | `[".luau"]` | File endings treated as sources. The longest matching ending is removed to get the node name. The leading dot is optional. |
| `init_files` | `init.luau`, `init.server.luau`, `init.client.luau` | A folder containing one of these is mounted as **one** node (`{ "$path": "<folder>" }`) instead of being walked. |
| `default_to` | `["shared"]` | Containers for files no rule decides. If there is no `shared` container, the first container is used. |
| `on_collision` | `"warn"` | When two entries produce the same node name: `"warn"` (keep the last, print a warning), `"error"` (stop), `"last-wins"` (keep the last silently). |
| `ignore` | `[]` | Glob patterns for paths that are never scanned (files or folders). |

### Extensions and Rojo-style names

By default a file `ui.client.luau` becomes the node `Ui.client`. To get Rojo-style names, list the longer endings too:

```toml
extensions = [".client.luau", ".server.luau", ".luau"]
```

Now `ui.client.luau` becomes `UI`, and rules can still match its full path (for example `**/*.client.luau`).

## Containers

A container is a named place in the generated tree where nodes get mounted. Rules refer to containers by name.

```toml
[containers.shared]
mount = "ReplicatedStorage"

[containers.server]
mount = "ServerScriptService"

[containers.client]
mount = "StarterPlayer/StarterPlayerScripts"
```

- If you define **no** containers you get `shared` (`ReplicatedStorage`) and `server` (`ServerScriptService`) as shown above.
- If you define any, only the ones you list exist (defaults are not merged in). Declare `shared` and `server` again if you still want them.
- You can have up to 64. Order of declaration is the order in the output.
- A mount can be a service, or a path below one (`ReplicatedStorage/Packages`).
- Missing parts of a `mount` path are created automatically. Known services (`ReplicatedStorage`, `ServerScriptService`, `StarterPlayer`, `Workspace`, ...) and `StarterPlayerScripts` / `StarterCharacterScripts` under `StarterPlayer` get their own name as `$className`; any other final segment becomes a `Folder`; any other intermediate node gets no class. Set the class of anything else with [`[nodes]`](#node-settings-nodes-class-names-properties-paths) or a [`template`](#template). Nodes that already exist (from a template or `[nodes]`) are never changed by mounting.

## Rules

Rules decide where nodes go, rename them, or attach properties.

```toml
[[rules]]
name = "admin tools"                  # optional label shown in warnings and `explain`
match = "src/gameplay/admin"          # glob, or a list of globs
to = ["server"]                       # containers; [] excludes the node everywhere
rename = "Staff"                      # optional: override the node name
set = { SomeProperty = "Value" }      # optional: becomes "$properties" on the node
class_name = "Configuration"          # optional: "$className" of the node
```

At least one of `to`, `rename`, `set` or `class_name` is required.

`class_name` replaces the `Folder` class of a folder node (for example to make a folder a `Model` or `ScreenGui`), or adds a `$className` next to the `$path` of a file or `init` folder. Like `rename` and `set`, it applies only to the node the rule matches, not to its children. If several rules match, the more specific one wins.

### Matching

`match` is a glob against the node's **own path** (for files, including the extension):

| Pattern | Matches |
|---|---|
| `src/gameplay/ui` | exactly that path (no wildcards = exact path) |
| `src/gameplay/*/server` | `*` stays inside one folder level |
| `**/*.spec.luau` | `**` crosses folder levels |
| `src/**/Constants.luau` | any `Constants.luau` below `src` |
| `src/{gameplay,ui}` | alternatives |
| `?`, `[abc]` | one character, a character class |

`src/gameplay/ui/**` matches everything *inside* `ui` but not `ui` itself; usually you want the plain path `src/gameplay/ui`.

A rule on a **folder** applies to every file below it. A rule on a **file** applies to that file only.

### Which rule wins

For each file OctaTree decides the destination in this order; the first that applies wins:

1. **Rules matching the file's own path.** If several match, the more specific one wins (the one with more literal, wildcard-free path segments); on a tie, the one written later wins. Their `to` and `rename` override each other, and their `set` tables are merged.
2. **Suffix rules** (see below).
3. **The nearest parent folder with a `to` rule.** A deeper folder rule replaces one from further up, regardless of the order they are written in.
4. **`default_to`.**

Folders that contain an `init` file are mounted as one node and are routed by the same four steps, using the folder name for suffix rules.

Example:

```toml
[[rules]]
match = "src/gameplay/match"
to = ["shared", "server"]      # everything under match goes to both...

[[rules]]
match = "src/gameplay/match/internal"
to = ["server"]                # ...except this folder, which is server only
```

If a `match` hits nothing, you get a warning (`rules[N] ... matched nothing`), which usually means a typo.

## Suffix rules

There are none by default. Add them to route by the end of a node's name (extension removed):

```toml
[[suffix_rules]]
suffix = ["server", "service"]   # one string or a list
to = ["server"]
case_insensitive = true          # default
```

The first suffix rule that matches wins, so order them from most to least specific when they can overlap. A file-specific rule beats a suffix rule, and a suffix rule beats a destination inherited from a folder.

Matching ignores case by default, so `server` also matches a name like `Observer`. Set `case_insensitive = false` and write the suffix as it appears (`"Server"`) to avoid that.

## Naming

```toml
[naming]
case = "pascal"
keep_prefixes = ["use"]
prefix_match = "word"
short_word_max = 2
```

| Key | Values | Description |
|---|---|---|
| `case` | `pascal` (default), `camel`, `snake`, `keep` | `damage_math` becomes `DamageMath`, `damageMath`, `damage_math`, or stays as is. |
| `keep_prefixes` | list of strings, default `[]` | Names starting with one of these keep their original casing. |
| `prefix_match` | `word` (default), `starts_with` | `word`: the prefix must be followed by an uppercase letter, digit, `_`, `-`, `.`, space or the end (`useThing` is kept with prefix `use`, `user` is not). `starts_with`: plain prefix test (`user` is kept too). |
| `short_word_max` | number, default `2` | Words with at most this many characters are written in capitals (`ui` becomes `UI`, `id` becomes `ID`). `0` turns this off. |

Words are split at capital letters, spaces, `_` and `-`. `DamageMath`, `damage_math` and `damage-math` all produce `DamageMath`.

To give one node a specific name, use a `rename` rule instead of changing global naming.

## Ignoring files

```toml
ignore = ["**/*.spec.luau", "src/gameplay/scratch"]
```

Ignored files and folders are skipped entirely, including everything inside an ignored folder.

## Node settings (`[nodes]`): class names, properties, paths

Some nodes of the project tree are not made of your files but still need a `$className`, `$properties` or a `$path`: services such as `HttpService`, the parents of a mount (`StarterPlayerScripts`), or `Workspace` settings. Describe them under `[nodes]`, keyed by their **path inside the tree** (use quotes when the path contains `/`):

```toml
[nodes.HttpService]
class_name = "HttpService"
properties = { HttpEnabled = true }

[nodes.StarterPlayer]
class_name = "StarterPlayer"

[nodes."StarterPlayer/StarterPlayerScripts"]
class_name = "StarterPlayerScripts"

[nodes.Workspace]
class_name = "Workspace"
properties = { Gravity = 67.3 }

[nodes."Workspace/terrain"]
path = "Terrain.rbxm"
```

| Key | Writes | Notes |
|---|---|---|
| `class_name` | `$className` | |
| `properties` | `$properties` | Merged into properties that already exist (from the template, for example) |
| `path` | `$path` | A file or folder on disk, relative to the project file, exactly as Rojo reads it |
| `ignore_unknown_instances` | `$ignoreUnknownInstances` | `true` / `false` |

At least one key is required. Missing nodes (and their parents) are created. Nodes are applied before your containers are mounted, so a container whose `mount` passes through one of them is placed inside it and keeps its class.

`[nodes]` also adds to the default tree and to a `[template]`; you don't need a template just to set a class.

To mount folders that are not part of your modules (packages, bootstrap scripts, assets), see [Extra folders](extra-folders.md).

## Extra project fields (`[rojo]`)

Anything under `[rojo]` is written as a top-level field of the project file, right after `name`. Use it for Rojo options such as `globIgnorePaths`, `placeId` or `servePort`:

```toml
[rojo]
globIgnorePaths = ["**/*.spec.lua"]
placeId = 1234567890
servePort = 34872
```

Keys are copied as written (Rojo expects camelCase). `name` and `tree` are not allowed here: use `project_name`, and the rest of your settings, for those. Note that `ignore` (in OctaTree's own config) decides which files OctaTree scans, while `globIgnorePaths` is a Rojo setting about which files Rojo syncs.

## Template

The template is the skeleton that your modules are mounted into. It is the **content of `tree`** in the generated project file. `$className = "DataModel"` is added if you don't set it.

Inline, as TOML (keys that start with `$` must be quoted):

```toml
[template.ReplicatedStorage]
"$ignoreUnknownInstances" = true

[template.ReplicatedStorage.Packages]
"$path" = "Packages"
"$className" = "Folder"

[template.ServerScriptService]
"$ignoreUnknownInstances" = true
```

Or from a JSON file next to the config:

```toml
template_file = "template.json"
```

Use one or the other, not both.

Without a template, the tree starts with `ReplicatedStorage` and `ServerScriptService`, each with `"$ignoreUnknownInstances": true`, if your containers are exactly the default `shared` and `server`. With other containers it starts as a bare `DataModel`. When you provide a template, it is used as is: it contains only what you put in it.

## Recipes

**A third place for client code**

```toml
modules = ["gameplay"]

[containers.shared]
mount = "ReplicatedStorage"
[containers.server]
mount = "ServerScriptService"
[containers.client]
mount = "StarterPlayer/StarterPlayerScripts"

[[rules]]
match = "src/gameplay/hud"
to = ["client"]
```

**Services named by suffix** (`PlayerServiceClient` on the client, `PlayerServiceServer` on the server)

```toml
[containers.shared]
mount = "ReplicatedStorage"
[containers.server]
mount = "ServerScriptService"
[containers.client]
mount = "StarterPlayer/StarterPlayerScripts"

[[suffix_rules]]
suffix = ["server"]
to = ["server"]

[[suffix_rules]]
suffix = ["client"]
to = ["client"]
```

**Plain files on both client and server by default**

```toml
default_to = ["client", "server"]

[containers.shared]
mount = "ReplicatedStorage"
[containers.server]
mount = "ServerScriptService"
[containers.client]
mount = "StarterPlayer/StarterPlayerScripts"
```

**Mount a folder in two containers**

```toml
[[rules]]
match = "src/gameplay/constants"
to = ["shared", "server"]
```

**Keep a folder out of the tree entirely**

```toml
[[rules]]
match = "src/gameplay/experiments"
to = []
```

**Give a folder a fixed name**

```toml
[[rules]]
match = "src/gameplay/net_stuff"
rename = "Net"
```

**Set a property on every client script**

```toml
extensions = [".client.luau", ".luau"]

[[rules]]
match = "**/*.client.luau"
set = { RunContext = "Client" }
```

## Warnings and errors

Errors stop generation (exit code 2). Warnings are printed to stderr; with `--strict` they also stop generation.

| Message | Meaning |
|---|---|
| `unknown field ...` | A key in the config is misspelled. |
| `unknown container ...` | A `to` or `default_to` names a container that isn't defined. |
| `rule has no effect` | A rule has none of `to`, `rename`, `set`, `class_name`. |
| `bad pattern ...` | A glob is malformed. |
| `[nodes] ... is not a table` | A `[nodes]` path runs through a value that is not a table (for example a template key set to a string). |
| `[rojo] cannot set name or tree` | Use `project_name` instead; `tree` is generated. |
| `matched nothing` (warning) | A rule never matched a path. |
| `module folder ... does not exist` (warning) | A `modules` entry has no folder. |
| `name collision` (warning, or error with `on_collision = "error"`) | Two entries produced the same node name. |
| `skipped non-UTF-8 name` (warning) | A file name that isn't valid Unicode was skipped. |

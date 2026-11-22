# Extra folders: packages, bootstrap scripts and other things outside your modules

`modules` are folders that OctaTree scans and routes file by file. Some things in a project are not like that: package folders (for example from Wally), bootstrap scripts, `.rbxm` assets. They should be mounted **as they are**, at a place and under a name you choose. That is what [`[nodes]`](configuration.md#node-settings-nodes-class-names-properties-paths) with a `path` is for.

## The idea

```toml
[nodes."<where in the Roblox tree>"]
path = "<where on disk>"
```

- The **key** is the location in the tree. Its last part is the **name of the instance**, so choosing a name is also how you rename: the folder `Packages` on disk can appear as `Packages`, `vendor`, or anything else.
- `path` is a file or folder on disk, relative to the project file (normally your project root, next to `octatree.toml`). Rojo syncs it as it is; OctaTree does not scan or route it, and it does not need to be listed in `modules`.
- Keys with a `/` must be quoted. Missing parents are created; known services (`ReplicatedStorage`, `ServerScriptService`, ...) get their class automatically.

## Packages (shared and server)

Packages that both client and server use go next to your source folder in `ReplicatedStorage`; server-only packages go in `ServerScriptService`:

```toml
[nodes."ReplicatedStorage/Packages"]
path = "Packages"

[nodes."ServerScriptService/ServerPackages"]
path = "ServerPackages"
```

To use another name in the game, change the last part of the key. This mounts the folder `Packages` as an instance called `vendor`:

```toml
[nodes."ReplicatedStorage/vendor"]
path = "Packages"
```

## Bootstrap scripts

A whole folder of scripts, mounted next to your modules inside the source folder:

```toml
[nodes."ReplicatedStorage/Source/Bootstrap"]
path = "bootstrap"
```

Or single scripts, each with its own name and properties (`properties` are written as Rojo's `$properties`):

```toml
[nodes."ReplicatedStorage/Source/ClientBootstrap"]
path = "bootstrap/client.server.luau"
properties = { RunContext = "Client" }

[nodes."ServerScriptService/Source/ServerBootstrap"]
path = "bootstrap/server.server.luau"
```

Here `Source` is the mount of your containers (see below). You don't have to declare it: a mount that only exists because of a node becomes a `Folder` automatically.

## All together

```toml
project_name = "MyGame"
modules = ["gameplay"]

[containers.shared]
mount = "ReplicatedStorage/Source"

[containers.server]
mount = "ServerScriptService/Source"

[[suffix_rules]]
suffix = ["server"]
to = ["server"]

[nodes.ReplicatedStorage]
ignore_unknown_instances = true

[nodes.ServerScriptService]
ignore_unknown_instances = true

[nodes."ReplicatedStorage/Packages"]
path = "Packages"

[nodes."ServerScriptService/ServerPackages"]
path = "ServerPackages"

[nodes."ReplicatedStorage/Source/ClientBootstrap"]
path = "bootstrap/client.server.luau"
properties = { RunContext = "Client" }

[nodes."ServerScriptService/Source/ServerBootstrap"]
path = "bootstrap/server.server.luau"
```

With `src/gameplay/Inventory.luau`, `src/gameplay/MatchServer.luau` and the folders above, `octatree generate` writes (shortened):

```json
{
  "name": "MyGame",
  "tree": {
    "$className": "DataModel",
    "ReplicatedStorage": {
      "$className": "ReplicatedStorage",
      "$ignoreUnknownInstances": true,
      "Packages": { "$path": "Packages" },
      "Source": {
        "$className": "Folder",
        "ClientBootstrap": {
          "$path": "bootstrap/client.server.luau",
          "$properties": { "RunContext": "Client" }
        },
        "Gameplay": {
          "$className": "Folder",
          "Inventory": { "$path": "src/gameplay/Inventory.luau" }
        }
      }
    },
    "ServerScriptService": {
      "$className": "ServerScriptService",
      "$ignoreUnknownInstances": true,
      "ServerPackages": { "$path": "ServerPackages" },
      "Source": {
        "$className": "Folder",
        "ServerBootstrap": { "$path": "bootstrap/server.server.luau" },
        "Gameplay": {
          "$className": "Folder",
          "MatchServer": { "$path": "src/gameplay/MatchServer.luau" }
        }
      }
    }
  }
}
```

The `[nodes.ReplicatedStorage]` and `[nodes.ServerScriptService]` blocks keep `$ignoreUnknownInstances` on the services. They are needed here because custom mount paths (`.../Source`) replace the minimal default tree, which has that flag already.

## Good to know

- **Names are used exactly as written.** Naming rules (`case`, `keep_prefixes`, ...) apply to files and folders found in your modules, not to `[nodes]` keys.
- **Don't reuse a name that a module produces** at the same place. If a node and a module folder end up with the same name in the same parent, the module's entry replaces the node.
- **OctaTree doesn't check that `path` exists.** Rojo reports a missing path when you build or serve.
- **Other properties** work the same way: `class_name`, `properties` and `ignore_unknown_instances` can be used on any node (for example to keep Studio-made instances inside a package folder).
- **Want routing for those files too?** If the folder should follow your rules like the rest of your code (suffixes, per-folder containers), move it under `src` and add it to `modules` instead.
- **Whole skeleton instead?** For a fixed tree you can also write a [template](configuration.md#template); `[nodes]` is for adding to the tree without writing all of it.

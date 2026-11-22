//! Behavior tests for the engine, driven by `octatree.toml` text and an
//! in-memory file tree.

use std::fs;
use std::path::Path;

use octatree::explain::explain;
use octatree::generate;
use octatree::import::Imported;
use octatree::migrate::to_toml;
use octatree::scan::MemReader;
use octatree::settings::{STARTER_TOML, Settings};
use serde_json::{Value, json};

fn run(toml: &str, files: &[&str]) -> (Value, Vec<String>) {
    let s = Settings::from_toml(toml, Path::new("octatree.toml")).expect("settings");
    let mut reader = MemReader::from_paths(files.iter().copied());
    let out = generate(&s, &mut reader).expect("generate");
    (serde_json::from_str(&out.json).unwrap(), out.warnings)
}

fn settings_err(toml: &str) -> String {
    Settings::from_toml(toml, Path::new("octatree.toml"))
        .expect_err("should be invalid")
        .to_string()
}

/// Names of the children (not `$` keys) at a path inside `tree`.
fn keys(v: &Value, path: &str) -> Vec<String> {
    let mut cur = &v["tree"];
    for seg in path.split('/') {
        cur = &cur[seg];
    }
    cur.as_object()
        .map(|m| m.keys().filter(|k| !k.starts_with('$')).cloned().collect())
        .unwrap_or_default()
}

const BASE: &str = "modules = [\"gameplay\"]\n";
/// Top-level key (must come before any `[table]`).
const SUFFIX: &str = "suffix_rules = [{ suffix = [\"server\", \"service\"], to = [\"server\"] }]\n";

// ----------------------------------------------------------------- defaults

#[test]
fn an_empty_config_produces_only_the_two_services() {
    let (v, warnings) = run("", &[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(v["name"], "Template");
    let tree = v["tree"].as_object().unwrap();
    let names: Vec<&String> = tree.keys().collect();
    assert_eq!(
        names,
        ["$className", "ReplicatedStorage", "ServerScriptService"]
    );
    assert_eq!(
        tree["ReplicatedStorage"],
        json!({ "$ignoreUnknownInstances": true })
    );
    assert_eq!(
        tree["ServerScriptService"],
        json!({ "$ignoreUnknownInstances": true })
    );
}

#[test]
fn nothing_is_scanned_until_modules_are_listed() {
    let (v, _) = run("", &["src/gameplay/A.luau"]);
    assert!(keys(&v, "ReplicatedStorage").is_empty());
}

#[test]
fn default_routing_is_shared_and_there_are_no_default_suffix_rules() {
    let (v, _) = run(
        BASE,
        &["src/gameplay/A.luau", "src/gameplay/FooServer.luau"],
    );
    assert_eq!(keys(&v, "ReplicatedStorage/Gameplay"), ["A", "FooServer"]);
    assert!(keys(&v, "ServerScriptService").is_empty());
}

#[test]
fn the_starter_config_is_valid_and_describes_no_structure() {
    let s = Settings::from_toml(STARTER_TOML, Path::new("octatree.toml")).unwrap();
    assert!(s.modules.is_empty() && s.rules.is_empty() && s.suffix_rules.is_empty());
    assert!(s.nodes.is_empty() && s.rojo.is_empty() && s.naming.keep_prefixes.is_empty());
    let (v, warnings) = run(STARTER_TOML, &["src/anything/A.luau"]);
    assert!(warnings.is_empty());
    assert_eq!(v["tree"].as_object().unwrap().len(), 3);
}

// ------------------------------------------------------------------ routing

#[test]
fn rule_beats_suffix_beats_inherited_beats_default() {
    let toml = format!(
        "{BASE}{SUFFIX}
[[rules]]
match = \"src/gameplay/dir\"
to = [\"shared\"]

[[rules]]
match = \"src/gameplay/dir/ForcedService.luau\"
to = [\"shared\"]
"
    );
    let (v, _) = run(
        &toml,
        &[
            "src/gameplay/dir/Plain.luau",
            "src/gameplay/dir/OtherService.luau",
            "src/gameplay/dir/ForcedService.luau",
            "src/gameplay/Free.luau",
            "src/gameplay/FreeServer.luau",
        ],
    );
    assert!(keys(&v, "ReplicatedStorage/Gameplay").contains(&"Free".into()));
    assert!(
        keys(&v, "ServerScriptService/Gameplay").contains(&"FreeServer".into()),
        "suffix beats default"
    );
    let dir_shared = keys(&v, "ReplicatedStorage/Gameplay/Dir");
    assert!(dir_shared.contains(&"Plain".into()));
    assert!(
        dir_shared.contains(&"ForcedService".into()),
        "file rule beats suffix"
    );
    assert!(
        keys(&v, "ServerScriptService/Gameplay/Dir").contains(&"OtherService".into()),
        "suffix beats inherited"
    );
}

#[test]
fn nearest_directory_rule_wins_regardless_of_order() {
    let toml = format!(
        "{BASE}
[[rules]]
match = \"src/gameplay/a/b\"
to = [\"server\"]

[[rules]]
match = \"src/gameplay/a\"
to = [\"shared\"]
"
    );
    let (v, _) = run(&toml, &["src/gameplay/a/b/X.luau", "src/gameplay/a/Y.luau"]);
    assert_eq!(keys(&v, "ServerScriptService/Gameplay/A/B"), ["X"]);
    assert_eq!(keys(&v, "ReplicatedStorage/Gameplay/A"), ["Y"]);
}

#[test]
fn glob_ignore_rename_and_properties() {
    let toml = format!(
        "{BASE}extensions = [\".client.luau\", \".luau\"]
ignore = [\"**/*.spec.luau\"]

[[rules]]
match = \"src/gameplay/net_stuff\"
rename = \"Net\"

[[rules]]
match = \"**/*.client.luau\"
set = {{ RunContext = \"Client\" }}
"
    );
    let (v, _) = run(
        &toml,
        &[
            "src/gameplay/net_stuff/A.luau",
            "src/gameplay/B.spec.luau",
            "src/gameplay/ui.client.luau",
        ],
    );
    let shared = &v["tree"]["ReplicatedStorage"]["Gameplay"];
    assert!(shared.get("Net").is_some(), "{shared}");
    assert!(shared.get("BSpec").is_none(), "ignored");
    assert_eq!(shared["UI"]["$properties"]["RunContext"], "Client");
}

#[test]
fn default_to_can_target_several_containers() {
    let toml = format!(
        "{BASE}default_to = [\"shared\", \"server\"]
"
    );
    let (v, _) = run(&toml, &["src/gameplay/sub/A.luau"]);
    assert_eq!(keys(&v, "ReplicatedStorage/Gameplay/Sub"), ["A"]);
    assert_eq!(keys(&v, "ServerScriptService/Gameplay/Sub"), ["A"]);
}

#[test]
fn init_folders_honor_rules_and_suffixes() {
    let toml = format!(
        "{BASE}{SUFFIX}
[[rules]]
match = \"src/gameplay/dup\"
to = [\"shared\", \"server\"]
"
    );
    let (v, _) = run(
        &toml,
        &[
            "src/gameplay/dup/init.luau",
            "src/gameplay/FooService/init.luau",
        ],
    );
    for service in ["ReplicatedStorage", "ServerScriptService"] {
        assert!(v["tree"][service]["Gameplay"]["Dup"].get("$path").is_some());
    }
    assert!(
        v["tree"]["ReplicatedStorage"]["Gameplay"]
            .get("FooService")
            .is_none()
    );
    assert!(
        v["tree"]["ServerScriptService"]["Gameplay"]
            .get("FooService")
            .is_some()
    );
}

#[test]
fn collision_policies() {
    let files = ["src/gameplay/foo_bar.luau", "src/gameplay/FooBar.luau"];
    let (_, warnings) = run(BASE, &files);
    assert!(
        warnings.iter().any(|w| w.contains("collision")),
        "{warnings:?}"
    );

    let (_, warnings) = run(&format!("{BASE}on_collision = \"last-wins\"\n"), &files);
    assert!(warnings.is_empty());

    let s = Settings::from_toml(
        &format!("{BASE}on_collision = \"error\"\n"),
        Path::new("t.toml"),
    )
    .unwrap();
    assert!(generate(&s, &mut MemReader::from_paths(files)).is_err());
}

// ---------------------------------------------- containers, mounts, classes

#[test]
fn extra_containers_and_inferred_mount_classes() {
    let toml = "modules = [\"gameplay\"]
default_to = [\"client\"]

[containers.shared]
mount = \"ReplicatedStorage\"
[containers.client]
mount = \"StarterPlayer/StarterPlayerScripts\"

[[rules]]
match = \"src/gameplay/both\"
to = [\"shared\", \"client\"]
";
    let (v, _) = run(toml, &["src/gameplay/both/X.luau", "src/gameplay/Y.luau"]);
    let t = &v["tree"];
    assert_eq!(t["StarterPlayer"]["$className"], "StarterPlayer");
    assert_eq!(
        t["StarterPlayer"]["StarterPlayerScripts"]["$className"],
        "StarterPlayerScripts"
    );
    let client = &t["StarterPlayer"]["StarterPlayerScripts"]["Gameplay"];
    assert!(
        client.get("Y").is_some() && client["Both"].get("X").is_some(),
        "{client}"
    );
    assert_eq!(t["ReplicatedStorage"]["$className"], "ReplicatedStorage");
    let shared = &t["ReplicatedStorage"]["Gameplay"];
    assert!(shared["Both"].get("X").is_some() && shared.get("Y").is_none());
}

#[test]
fn mount_parents_get_inferred_classes_only_when_known() {
    let toml = r#"
modules = ["gameplay"]
default_to = ["a"]

[containers.a]
mount = "Workspace/Models/Packages"

[containers.b]
mount = "Custom/Packages"

[containers.c]
mount = "StarterPlayer/StarterCharacterScripts/Packages"

[containers.d]
mount = "SoundService"

[[rules]]
match = "src/gameplay/b"
to = ["b"]

[[rules]]
match = "src/gameplay/c"
to = ["c"]

[[rules]]
match = "src/gameplay/d"
to = ["d"]

[nodes."StarterPlayer/StarterCharacterScripts"]
class_name = "StarterCharacterScripts"
properties = { Marker = true }
"#;
    let (v, _) = run(
        toml,
        &[
            "src/gameplay/a/X.luau",
            "src/gameplay/b/Y.luau",
            "src/gameplay/c/Z.luau",
            "src/gameplay/d/W.luau",
        ],
    );
    let t = &v["tree"];
    assert_eq!(t["Workspace"]["$className"], "Workspace");
    assert_eq!(
        t["Workspace"]["Models"].get("$className"),
        None,
        "unknown parent: no class"
    );
    assert_eq!(t["Workspace"]["Models"]["Packages"]["$className"], "Folder");
    assert_eq!(
        t["Custom"].get("$className"),
        None,
        "unknown top-level name: no class"
    );
    // A user-provided node is left exactly as configured.
    let chars = &t["StarterPlayer"]["StarterCharacterScripts"];
    assert_eq!(chars["$className"], "StarterCharacterScripts");
    assert_eq!(chars["$properties"]["Marker"], true);
    assert_eq!(t["StarterPlayer"]["$className"], "StarterPlayer");
    // A bare service as the mount is a service, not a Folder.
    assert_eq!(t["SoundService"]["$className"], "SoundService");
    assert!(t["SoundService"]["Gameplay"]["D"].get("W").is_some());
}

#[test]
fn custom_template_replaces_the_default_tree_and_unused_rules_warn() {
    let toml = format!(
        "{BASE}
[[rules]]
match = \"src/nothing/here\"
to = [\"server\"]

[template.ReplicatedStorage]
\"$ignoreUnknownInstances\" = true
\"$className\" = \"ReplicatedStorage\"
"
    );
    let (v, warnings) = run(&toml, &["src/gameplay/A.luau"]);
    assert_eq!(v["tree"]["$className"], "DataModel");
    assert_eq!(
        v["tree"]["ReplicatedStorage"]["$ignoreUnknownInstances"],
        true
    );
    assert!(
        v["tree"]["ReplicatedStorage"]["Gameplay"]
            .get("A")
            .is_some()
    );
    assert!(
        v["tree"].get("ServerScriptService").is_none(),
        "a custom template has only what you put in it"
    );
    assert!(
        warnings.iter().any(|w| w.contains("matched nothing")),
        "{warnings:?}"
    );
}

#[test]
fn rules_can_set_class_name_on_folders_and_files() {
    let toml = format!(
        "{BASE}
[[rules]]
match = \"src/gameplay/hud\"
class_name = \"ScreenGui\"

[[rules]]
match = \"src/gameplay/Data.luau\"
class_name = \"ModuleScript\"
"
    );
    let (v, _) = run(
        &toml,
        &["src/gameplay/hud/A.luau", "src/gameplay/Data.luau"],
    );
    let g = &v["tree"]["ReplicatedStorage"]["Gameplay"];
    assert_eq!(g["Hud"]["$className"], "ScreenGui");
    assert_eq!(g["Data"]["$className"], "ModuleScript");
    assert_eq!(g["Data"]["$path"], "src/gameplay/Data.luau");
}

#[test]
fn nodes_and_rojo_fields_with_a_full_example() {
    let toml = r#"
project_name = "Example"
modules = ["gameplay"]

[rojo]
globIgnorePaths = ["**/*.spec.lua"]

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

[containers.client]
mount = "StarterPlayer/StarterPlayerScripts/Packages"
"#;
    let (v, warnings) = run(toml, &["src/gameplay/A.luau"]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let top: Vec<&String> = v.as_object().unwrap().keys().collect();
    assert_eq!(top, ["name", "globIgnorePaths", "tree"]);
    assert_eq!(v["globIgnorePaths"][0], "**/*.spec.lua");

    let t = &v["tree"];
    assert_eq!(t["HttpService"]["$className"], "HttpService");
    assert_eq!(t["HttpService"]["$properties"]["HttpEnabled"], true);
    assert_eq!(t["Workspace"]["$properties"]["Gravity"], 67.3);
    assert_eq!(t["Workspace"]["terrain"]["$path"], "Terrain.rbxm");
    let scripts = &t["StarterPlayer"]["StarterPlayerScripts"];
    assert_eq!(t["StarterPlayer"]["$className"], "StarterPlayer");
    assert_eq!(scripts["$className"], "StarterPlayerScripts");
    assert_eq!(scripts["Packages"]["$className"], "Folder");
    assert!(scripts["Packages"]["Gameplay"].get("A").is_some());
}

#[test]
fn nodes_merge_with_existing_template_properties() {
    let toml = r#"
modules = ["gameplay"]

[template.Workspace]
"$className" = "Workspace"
"$properties" = { Gravity = 10 }

[nodes.Workspace]
properties = { FallenPartsDestroyHeight = -100 }
"#;
    let (v, _) = run(toml, &["src/gameplay/A.luau"]);
    let props = &v["tree"]["Workspace"]["$properties"];
    assert_eq!(props["Gravity"], 10);
    assert_eq!(props["FallenPartsDestroyHeight"], -100);
}

// -------------------------------------------------------------- validation

#[test]
fn settings_validation_errors_are_specific() {
    assert!(settings_err("modulez = []").contains("modulez"));
    assert!(
        settings_err("[[rules]]\nmatch = \"a\"\nto = [\"nope\"]")
            .contains("unknown container `nope`")
    );
    assert!(settings_err("[[rules]]\nmatch = \"a\"").contains("no effect"));
    assert!(settings_err("[[rules]]\nmatch = \"[\"\nto = [\"shared\"]").contains("bad pattern"));
    assert!(settings_err("version = 2").contains("unsupported version"));
    assert!(settings_err("modules = [\"a/b\"]").contains("single folder"));
    assert!(settings_err("[containers.x]\nmount = \"\"").contains("mount"));
    assert!(settings_err("[nodes.Workspace]").contains("at least one of"));
    assert!(settings_err("[nodes.Workspace]\nclassname = \"X\"").contains("classname"));
    assert!(settings_err("[rojo]\nname = \"X\"").contains("cannot set"));
    assert!(settings_err("[rojo]\ntree = {}").contains("cannot set"));
}

// ------------------------------------------------------------------ explain

#[test]
fn explain_reports_the_deciding_rule() {
    let toml = format!(
        "{BASE}{SUFFIX}
[[rules]]
match = \"src/gameplay/ui\"
to = [\"shared\"]
"
    );
    let s = Settings::from_toml(&toml, Path::new("t.toml")).unwrap();
    let mut reader = MemReader::from_paths([
        "src/gameplay/ui/Hud.luau",
        "src/gameplay/ui/ClockService.luau",
    ]);
    let text = explain(&s, "src/gameplay/ui/Hud.luau", &mut reader).unwrap();
    assert!(text.contains("=> [shared] via inherited"), "{text}");
    assert!(text.contains("src/gameplay/ui"), "{text}");
    let text = explain(&s, "src/gameplay/ui/ClockService.luau", &mut reader).unwrap();
    assert!(text.contains("=> [server] via suffix"), "{text}");
    let text = explain(&s, "src/other/X.luau", &mut reader).unwrap();
    assert!(text.contains("not in `modules`"), "{text}");
}

// ----------------------------------------------------------- import/migrate

#[test]
fn an_imported_old_config_routes_the_same_way() {
    let json = r#"{
        "context-server": [{"$path": "src/gameplay/srv", "$name": "Srv"}],
        "server-black-list": [{"$path": "src/gameplay/ui"}],
        "duplicated-modules": [{"$path": "src/gameplay/both"}],
        "dontUsePascalCase": {"prefixes": ["use"]}
    }"#;
    let imported = Imported::from_json(json, Path::new("old.json")).unwrap();
    let toml = to_toml(&imported).replace("modules = []", "modules = [\"gameplay\"]");
    let (v, warnings) = run(
        &toml,
        &[
            "src/gameplay/srv/A.luau",
            "src/gameplay/ui/B.luau",
            "src/gameplay/ui/PanelService.luau",
            "src/gameplay/both/C.luau",
            "src/gameplay/Plain.luau",
            "src/gameplay/DataService.luau",
            "src/gameplay/user_thing.luau",
        ],
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    // Server only: the renamed folder, and any name ending in Service/Server.
    assert_eq!(keys(&v, "ServerScriptService/Gameplay/Srv"), ["A"]);
    assert!(keys(&v, "ServerScriptService/Gameplay").contains(&"DataService".into()));
    assert_eq!(
        keys(&v, "ServerScriptService/Gameplay/UI"),
        ["PanelService"]
    );
    // Shared only: the "not on the server" folder; defaults.
    assert_eq!(keys(&v, "ReplicatedStorage/Gameplay/UI"), ["B"]);
    assert!(keys(&v, "ReplicatedStorage/Gameplay").contains(&"Plain".into()));
    // Both.
    assert_eq!(keys(&v, "ReplicatedStorage/Gameplay/Both"), ["C"]);
    assert_eq!(keys(&v, "ServerScriptService/Gameplay/Both"), ["C"]);
    // The old plain "starts with" prefix matching is kept.
    assert!(keys(&v, "ReplicatedStorage/Gameplay").contains(&"user_thing".into()));
}

// ---------------------------------------------------------------- file system

#[test]
fn fs_reader_treats_a_file_as_not_a_directory() {
    use octatree::scan::{DirReader, FsReader};
    let dir = std::env::temp_dir().join(format!("octatree-reader-test-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a.txt"), "").unwrap();
    let mut reader = FsReader::new(&dir);
    assert!(reader.read_dir("a.txt").unwrap().is_none());
    assert!(reader.read_dir("missing").unwrap().is_none());
    assert_eq!(reader.read_dir("").unwrap().unwrap().len(), 1);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn extra_folders_can_live_inside_or_beside_a_mount() {
    let toml = r#"
modules = ["gameplay"]

[containers.shared]
mount = "ReplicatedStorage/Source"

[containers.server]
mount = "ServerScriptService/Source"

[nodes."ReplicatedStorage/Packages"]
path = "Packages"

[nodes."ReplicatedStorage/vendor"]
path = "Packages"

[nodes."ServerScriptService/ServerPackages"]
path = "ServerPackages"

[nodes."ReplicatedStorage/Source/Bootstrap"]
path = "bootstrap"

[nodes."ServerScriptService/Source/Start"]
path = "bootstrap/Start.server.luau"
properties = { RunContext = "Server" }
"#;
    let (v, warnings) = run(toml, &["src/gameplay/A.luau"]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let t = &v["tree"];

    // Beside the mount: any instance name, pointing at any folder on disk.
    assert_eq!(t["ReplicatedStorage"]["Packages"]["$path"], "Packages");
    assert_eq!(
        t["ReplicatedStorage"]["vendor"]["$path"], "Packages",
        "renamed"
    );
    assert_eq!(
        t["ServerScriptService"]["ServerPackages"]["$path"],
        "ServerPackages"
    );
    assert_eq!(t["ReplicatedStorage"]["$className"], "ReplicatedStorage");

    // Inside the mount: the mount becomes a Folder, first key, and keeps both.
    let source = &t["ReplicatedStorage"]["Source"];
    let order: Vec<&String> = source.as_object().unwrap().keys().collect();
    assert_eq!(order, ["$className", "Bootstrap", "Gameplay"]);
    assert_eq!(source["$className"], "Folder");
    assert_eq!(source["Bootstrap"]["$path"], "bootstrap");

    let server_source = &t["ServerScriptService"]["Source"];
    assert_eq!(server_source["$className"], "Folder");
    assert_eq!(
        server_source["Start"]["$properties"]["RunContext"],
        "Server"
    );
    assert!(
        server_source.get("Gameplay").is_none(),
        "nothing routed to the server"
    );
}

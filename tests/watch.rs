//! `octatree watch` / `octatree stop`: control files and the watcher.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use octatree::watch::{Control, Watcher};
use serde_json::Value;

fn temp_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("octatree-test-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir).unwrap();
    dir
}

fn write(path: &Path, content: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, content).unwrap();
}

#[test]
fn control_lifecycle() {
    let dir = temp_dir("control");
    let control = Control::in_dir(&dir, "abc");
    assert!(!control.is_active(), "no watcher yet");
    assert!(!control.stop_requested());

    {
        let _guard = control.start().unwrap();
        assert!(
            control.is_active(),
            "a fresh heartbeat means a watcher is running"
        );
        assert!(!control.stop_requested());

        // `octatree stop` from another process (same files, new handle).
        let other = Control::in_dir(&dir, "abc");
        assert!(other.is_active());
        other.request_stop().unwrap();
        assert!(control.stop_requested());
    }

    // The guard removed both files.
    assert!(!control.is_active());
    assert!(!control.stop_requested());
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_stale_heartbeat_is_not_active_and_a_new_watcher_clears_old_stop_requests() {
    let dir = temp_dir("stale");
    let control = Control::in_dir(&dir, "abc");
    control.beat().unwrap();
    assert!(control.is_active());

    // Pretend the watcher died a minute ago.
    let lock = dir.join("octatree-watch-abc.lock");
    let file = fs::OpenOptions::new().write(true).open(&lock).unwrap();
    file.set_modified(SystemTime::now() - Duration::from_secs(60))
        .unwrap();
    drop(file);
    assert!(!control.is_active());

    // A leftover stop marker must not stop the next watcher immediately.
    control.request_stop().unwrap();
    let guard = control.start().unwrap();
    assert!(!control.stop_requested());
    assert!(control.is_active());
    drop(guard);
    fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn different_projects_do_not_share_control_files() {
    let a = temp_dir("proj-a");
    let b = temp_dir("proj-b");
    let ca = Control::for_root(&a);
    let cb = Control::for_root(&b);
    let _guard = ca.start().unwrap();
    assert!(ca.is_active());
    assert!(!cb.is_active());
    fs::remove_dir_all(&a).unwrap();
    fs::remove_dir_all(&b).unwrap();
}

fn project_json(path: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

fn tick(watcher: &mut Watcher) -> Vec<String> {
    let mut log = Vec::new();
    watcher.tick(&mut |m| log.push(m));
    log
}

#[test]
fn the_watcher_follows_files_and_configuration() {
    let root = temp_dir("watcher");
    write(&root.join("octatree.toml"), "modules = [\"gameplay\"]\n");
    write(&root.join("src/gameplay/A.luau"), "");
    let out = root.join("default.project.json");

    let mut watcher =
        Watcher::new(root.clone(), root.join("octatree.toml"), None).expect("watcher");
    assert_eq!(watcher.output_path(), out);

    // First tick writes the file.
    let log = tick(&mut watcher);
    assert!(log.iter().any(|m| m.contains("generated")), "{log:?}");
    let shared = |v: &Value| v["tree"]["ReplicatedStorage"]["Gameplay"].clone();
    assert!(shared(&project_json(&out)).get("A").is_some());

    // Nothing changed: silence.
    assert!(tick(&mut watcher).is_empty());

    // A new file shows up.
    write(&root.join("src/gameplay/B.luau"), "");
    let log = tick(&mut watcher);
    assert!(log.iter().any(|m| m.contains("generated")), "{log:?}");
    assert!(shared(&project_json(&out)).get("B").is_some());
    assert!(tick(&mut watcher).is_empty());

    // A removed file disappears.
    fs::remove_file(root.join("src/gameplay/A.luau")).unwrap();
    tick(&mut watcher);
    assert!(shared(&project_json(&out)).get("A").is_none());

    // A config change is picked up (route everything to the server).
    write(
        &root.join("octatree.toml"),
        "modules = [\"gameplay\"]\ndefault_to = [\"server\"]\n",
    );
    let log = tick(&mut watcher);
    assert!(
        log.iter().any(|m| m.contains("configuration reloaded")),
        "{log:?}"
    );
    let v = project_json(&out);
    assert!(
        v["tree"]["ServerScriptService"]["Gameplay"]
            .get("B")
            .is_some()
    );
    assert!(v["tree"]["ReplicatedStorage"].get("Gameplay").is_none());

    // A broken config is reported once, and the last good output stays.
    write(&root.join("octatree.toml"), "modulez = []\n");
    let log = tick(&mut watcher);
    assert!(
        log.iter()
            .any(|m| m.starts_with("error:") && m.contains("modulez")),
        "{log:?}"
    );
    assert!(
        tick(&mut watcher).is_empty(),
        "the same error is not repeated"
    );
    assert!(
        project_json(&out)["tree"]["ServerScriptService"]["Gameplay"]
            .get("B")
            .is_some()
    );

    // Fixing the config recovers.
    write(&root.join("octatree.toml"), "modules = [\"gameplay\"]\n");
    let log = tick(&mut watcher);
    assert!(
        log.iter().any(|m| m.contains("configuration reloaded")),
        "{log:?}"
    );
    assert!(shared(&project_json(&out)).get("B").is_some());

    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn the_watcher_reports_warnings_once_and_honors_the_out_override() {
    let root = temp_dir("watcher-warn");
    write(
        &root.join("octatree.toml"),
        "modules = [\"gameplay\", \"missing\"]\n",
    );
    write(&root.join("src/gameplay/A.luau"), "");
    let out = root.join("custom.json");

    let mut watcher =
        Watcher::new(root.clone(), root.join("octatree.toml"), Some(out.clone())).unwrap();
    let log = tick(&mut watcher);
    assert!(
        log.iter()
            .any(|m| m.contains("warning") && m.contains("missing")),
        "{log:?}"
    );
    assert!(out.is_file());
    assert!(!root.join("default.project.json").exists());
    assert!(tick(&mut watcher).is_empty(), "warnings are not repeated");
    fs::remove_dir_all(&root).unwrap();
}

#[test]
fn a_missing_or_invalid_config_fails_at_startup() {
    let root = temp_dir("watcher-bad");
    assert!(Watcher::new(root.clone(), root.join("octatree.toml"), None).is_err());
    write(&root.join("octatree.toml"), "bogus = 1\n");
    assert!(Watcher::new(root.clone(), root.join("octatree.toml"), None).is_err());
    fs::remove_dir_all(&root).unwrap();
}

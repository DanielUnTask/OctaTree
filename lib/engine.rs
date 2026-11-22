//! The modern engine: walks the modules, applies the rules from
//! [`Settings`], and mounts the result into every configured container.
//!
//! Pure given a [`DirReader`]; all routing decisions live in [`Settings`].

use serde_json::{Map, Value, json};

use crate::error::{Error, Result};
use crate::scan::{DirReader, Entry, compare_names};
use crate::settings::{Direct, Mask, NodeSpec, OnCollision, Settings};

/// Result of [`build`].
#[derive(Debug)]
pub struct Built {
    /// The full project file contents.
    pub tree: Value,
    /// Non-fatal problems (name collisions, rules that matched nothing, ...).
    pub warnings: Vec<String>,
}

/// The built-in starting point: only the two services, each with
/// `$ignoreUnknownInstances`. Used when the containers are the default two
/// and no `template` is configured.
fn default_scaffold(name: &str) -> Value {
    json!({
        "name": name,
        "tree": {
            "$className": "DataModel",
            "ReplicatedStorage": { "$ignoreUnknownInstances": true },
            "ServerScriptService": { "$ignoreUnknownInstances": true }
        }
    })
}

struct Walker<'a, R: DirReader> {
    s: &'a Settings,
    reader: &'a mut R,
    /// Current project-relative posix path, grown/truncated while walking.
    path: String,
    warnings: Vec<String>,
    /// Scratch buffer for glob matches.
    buf: Vec<usize>,
    /// Per-rule hit counters (unused-rule detection).
    hits: Vec<usize>,
}

fn leaf(path: &str, d: &Direct) -> Value {
    let mut m = Map::new();
    m.insert("$path".into(), path.into());
    if let Some(class) = &d.class_name {
        m.insert("$className".into(), class.as_str().into());
    }
    if let Some(set) = &d.set {
        m.insert("$properties".into(), Value::Object(set.clone()));
    }
    Value::Object(m)
}

fn folder(children: Map<String, Value>, d: &Direct) -> Value {
    let mut m = Map::with_capacity(children.len() + 2);
    let class = d.class_name.as_deref().unwrap_or("Folder");
    m.insert("$className".into(), class.into());
    if let Some(set) = &d.set {
        m.insert("$properties".into(), Value::Object(set.clone()));
    }
    m.extend(children);
    Value::Object(m)
}

fn in_mask(mask: Mask, i: usize) -> bool {
    (mask >> i) & 1 == 1
}

impl<R: DirReader> Walker<'_, R> {
    fn direct_here(&mut self) -> Direct {
        let d = self.s.direct(&self.path, &mut self.buf);
        for &r in &d.rules {
            self.hits[r] += 1;
        }
        d
    }

    fn insert_node(
        &mut self,
        map: &mut Map<String, Value>,
        name: String,
        node: Value,
    ) -> Result<()> {
        if map.contains_key(&name) {
            let msg = format!("more than one entry maps to `{name}` near {}", self.path);
            match self.s.on_collision {
                OnCollision::Error => return Err(Error::Collision(msg)),
                OnCollision::Warn => self
                    .warnings
                    .push(format!("name collision: {msg} (last wins)")),
                OnCollision::LastWins => {}
            }
        }
        map.insert(name, node);
        Ok(())
    }

    /// Routes one directory. `d` holds the rules matching the directory itself.
    fn visit_dir(
        &mut self,
        dir_name: &str,
        d: &Direct,
        mut entries: Vec<Entry>,
        inherited: Option<Mask>,
    ) -> Result<Vec<Option<Value>>> {
        let s = self.s;
        let n = s.containers.len();

        if entries.iter().any(|e| s.is_init_file(&e.name)) {
            let (mask, _) = s.decide(dir_name, d, inherited);
            return Ok((0..n)
                .map(|i| in_mask(mask, i).then(|| leaf(&self.path, d)))
                .collect());
        }

        entries.sort_by(|a, b| compare_names(&a.name, &b.name));
        let child_inherited = d.to.map(|(m, _)| m).or(inherited);
        let mut maps: Vec<Map<String, Value>> = (0..n).map(|_| Map::new()).collect();
        let base_len = self.path.len();

        for entry in entries {
            self.path.push('/');
            self.path.push_str(&entry.name);

            if s.is_ignored(&self.path) {
                self.path.truncate(base_len);
                continue;
            }

            if entry.is_dir {
                let cd = self.direct_here();
                let name = s.node_name(&entry.name, &cd);
                let children = self
                    .reader
                    .read_dir(&self.path)?
                    .ok_or_else(|| Error::MissingDir(self.path.clone()))?;
                let out = self.visit_dir(&entry.name, &cd, children, child_inherited)?;
                self.path.truncate(base_len);
                for (i, node) in out.into_iter().enumerate() {
                    if let Some(v) = node {
                        self.insert_node(&mut maps[i], name.clone(), v)?;
                    }
                }
            } else if let Some(stem) = s.strip_extension(&entry.name) {
                let fd = self.direct_here();
                let (mask, _) = s.decide(stem, &fd, child_inherited);
                let name = s.node_name(stem, &fd);
                let node = leaf(&self.path, &fd);
                self.path.truncate(base_len);
                for (i, map) in maps.iter_mut().enumerate() {
                    if in_mask(mask, i) {
                        self.insert_node(map, name.clone(), node.clone())?;
                    }
                }
            } else {
                self.path.truncate(base_len);
            }
        }

        Ok(maps
            .into_iter()
            .map(|m| (!m.is_empty()).then(|| folder(m, d)))
            .collect())
    }
}

/// Top-level instances whose class name equals their name.
const SERVICES: [&str; 19] = [
    "Chat",
    "HttpService",
    "Lighting",
    "LocalizationService",
    "MaterialService",
    "ReplicatedFirst",
    "ReplicatedStorage",
    "ServerScriptService",
    "ServerStorage",
    "SoundService",
    "StarterGui",
    "StarterPack",
    "StarterPlayer",
    "Teams",
    "TestService",
    "TextChatService",
    "VoiceChatService",
    "Workspace",
    "MarketplaceService",
];

/// Children of `StarterPlayer` whose class name equals their name.
const PLAYER_SCRIPT_CONTAINERS: [&str; 2] = ["StarterPlayerScripts", "StarterCharacterScripts"];

/// Class to give a missing mount segment: a known service at the top level or
/// a `StarterPlayer` script container; nothing otherwise.
fn inferred_class(depth: usize, seg: &str) -> Option<&str> {
    let known = if depth == 0 {
        SERVICES.contains(&seg)
    } else {
        depth == 1 && PLAYER_SCRIPT_CONTAINERS.contains(&seg)
    };
    known.then_some(seg)
}

/// Inserts `children` at `segs` inside the project's `tree`. Missing nodes are
/// created: known services and `StarterPlayerScripts`-style containers with
/// their own class name, any other final (mount) segment as a `Folder`, any
/// other intermediate node without a class (set it with `[nodes]` or a
/// `template`). Nodes that already exist are left as they are.
fn mount(project: &mut Value, segs: &[String], children: Map<String, Value>) -> Result<()> {
    let mut cur = project
        .get_mut("tree")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| Error::Template("project has no `tree` object".into()))?;
    for (depth, seg) in segs.iter().enumerate() {
        let is_mount = depth + 1 == segs.len();
        let node = cur.entry(seg.clone()).or_insert_with(|| {
            if let Some(class) = inferred_class(depth, seg) {
                json!({ "$className": class })
            } else if is_mount {
                json!({ "$className": "Folder" })
            } else {
                json!({})
            }
        });
        cur = node.as_object_mut().ok_or_else(|| {
            Error::Template(format!(
                "`{}` in the template is not a table",
                segs[..=depth].join("/")
            ))
        })?;
    }
    ensure_mount_class(cur, segs);
    cur.extend(children);
    Ok(())
}

/// A mount that was created by `[nodes]` (e.g. a node placed inside it) has no
/// class yet: make it a `Folder` (as the first key), unless it is a known
/// service or already has a class or a path.
fn ensure_mount_class(mount: &mut Map<String, Value>, segs: &[String]) {
    let Some(last) = segs.last() else { return };
    if inferred_class(segs.len() - 1, last).is_none()
        && !mount.contains_key("$className")
        && !mount.contains_key("$path")
    {
        let existing = std::mem::take(mount);
        mount.insert("$className".into(), "Folder".into());
        mount.extend(existing);
    }
}

/// Like [`ensure_mount_class`] for a mount that receives no files: only acts
/// when the mount node already exists.
fn classify_existing_mount(project: &mut Value, segs: &[String]) {
    let Some(mut cur) = project.get_mut("tree").and_then(Value::as_object_mut) else {
        return;
    };
    for seg in segs {
        match cur.get_mut(seg).and_then(Value::as_object_mut) {
            Some(next) => cur = next,
            None => return,
        }
    }
    ensure_mount_class(cur, segs);
}

fn tree_object(project: &mut Value) -> Result<&mut Map<String, Value>> {
    project
        .get_mut("tree")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| Error::Template("project has no `tree` object".into()))
}

/// Applies the `[nodes]` settings: creates the addressed nodes if missing and
/// sets class, ignore flag, `$path` and (merged) properties.
fn apply_nodes(project: &mut Value, nodes: &[NodeSpec]) -> Result<()> {
    for spec in nodes {
        let mut cur = tree_object(project)?;
        for (depth, seg) in spec.path.iter().enumerate() {
            let node = cur.entry(seg.clone()).or_insert_with(|| {
                inferred_class(depth, seg).map_or_else(|| json!({}), |c| json!({ "$className": c }))
            });
            cur = node.as_object_mut().ok_or_else(|| {
                Error::Template(format!(
                    "`{}` is not a table, so `[nodes]` cannot configure `{}`",
                    spec.path[..=depth].join("/"),
                    spec.path.join("/")
                ))
            })?;
        }
        if let Some(class) = &spec.class_name {
            cur.insert("$className".into(), class.as_str().into());
        }
        if let Some(flag) = spec.ignore_unknown_instances {
            cur.insert("$ignoreUnknownInstances".into(), flag.into());
        }
        if let Some(path) = &spec.fs_path {
            cur.insert("$path".into(), path.as_str().into());
        }
        if let Some(props) = &spec.properties {
            let merged = cur
                .entry("$properties")
                .or_insert_with(|| json!({}))
                .as_object_mut()
                .ok_or_else(|| {
                    Error::Template(format!(
                        "`$properties` of `{}` is not a table",
                        spec.path.join("/")
                    ))
                })?;
            for (k, v) in props {
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    Ok(())
}

/// Places the `[rojo]` fields right after `name`.
fn with_rojo_fields(project: Value, rojo: &Map<String, Value>) -> Value {
    let Value::Object(mut obj) = project else {
        return project;
    };
    if rojo.is_empty() {
        return Value::Object(obj);
    }
    let mut out = Map::with_capacity(obj.len() + rojo.len());
    if let Some(name) = obj.remove("name") {
        out.insert("name".into(), name);
    }
    out.extend(rojo.clone());
    out.extend(obj);
    Value::Object(out)
}

/// Builds the project tree for `s` from the directories exposed by `reader`.
pub fn build<R: DirReader>(s: &Settings, reader: &mut R) -> Result<Built> {
    let mut walker = Walker {
        s,
        reader,
        path: String::new(),
        warnings: Vec::new(),
        buf: Vec::new(),
        hits: vec![0; s.rules.len()],
    };

    let n = s.containers.len();
    let mut roots: Vec<Map<String, Value>> = (0..n).map(|_| Map::new()).collect();
    for module in &s.modules {
        let root_path = format!("{}/{module}", s.source_dir);
        let Some(entries) = walker.reader.read_dir(&root_path)? else {
            walker
                .warnings
                .push(format!("module folder `{root_path}` does not exist"));
            continue;
        };
        walker.path = root_path;
        let d = walker.direct_here();
        let key = s.node_name(module, &d);
        let out = walker.visit_dir(module, &d, entries, None)?;
        for (i, node) in out.into_iter().enumerate() {
            if let Some(v) = node {
                roots[i].insert(key.clone(), v);
            }
        }
    }

    let mut project = match &s.template {
        Some(t) => {
            let mut tree = Map::new();
            if !t.contains_key("$className") {
                tree.insert("$className".into(), "DataModel".into());
            }
            tree.extend(t.clone());
            json!({ "name": s.project_name, "tree": Value::Object(tree) })
        }
        None if s.default_containers => default_scaffold(&s.project_name),
        None => json!({ "name": s.project_name, "tree": { "$className": "DataModel" } }),
    };

    apply_nodes(&mut project, &s.nodes)?;

    for (container, children) in s.containers.iter().zip(roots) {
        if children.is_empty() {
            classify_existing_mount(&mut project, &container.mount);
        } else {
            mount(&mut project, &container.mount, children)?;
        }
    }
    let project = with_rojo_fields(project, &s.rojo);

    let mut warnings = walker.warnings;
    for (rule, hits) in s.rules.iter().zip(&walker.hits) {
        if *hits == 0 {
            warnings.push(format!("{} matched nothing", rule.label));
        }
    }
    Ok(Built {
        tree: project,
        warnings,
    })
}

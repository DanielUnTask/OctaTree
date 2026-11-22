//! Modern configuration (`octatree.toml`): schema, validation and the pure
//! routing decision functions used by the engine and by `explain`.
//!
//! Routing model, per node (file, `init` folder):
//! 1. a rule whose `match` glob hits the node's own path (layered: more
//!    specific, then later, rules override earlier ones);
//! 2. a `suffix_rules` entry matching the node name;
//! 3. the target inherited from the nearest ancestor directory rule;
//! 4. `default_to`.

use std::path::Path;

use globset::{GlobBuilder, GlobSet, GlobSetBuilder};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::naming::{CaseStyle, NamingRules, PrefixMatch};

/// Bit `i` set means container `i` receives the node.
pub type Mask = u64;

const MAX_CONTAINERS: usize = 64;
const DEFAULT_MOUNTS: [(&str, &str); 2] = [
    ("shared", "ReplicatedStorage"),
    ("server", "ServerScriptService"),
];

/// What to do when two entries map to the same node name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OnCollision {
    /// Keep the last one and report a warning.
    #[default]
    Warn,
    /// Abort generation.
    Error,
    /// Keep the last one silently.
    LastWins,
}

/// A named mount point in the generated tree.
#[derive(Debug, Clone)]
pub struct Container {
    /// Name used by rules (`to = ["shared"]`).
    pub name: String,
    /// Path segments of the mount inside the project tree.
    pub mount: Vec<String>,
}

/// Suffix-based routing (e.g. names ending in `Server`).
#[derive(Debug, Clone)]
pub struct SuffixRule {
    /// Suffixes, lower-cased when `case_insensitive`.
    pub suffixes: Vec<String>,
    /// Compare ignoring case.
    pub case_insensitive: bool,
    /// Destination containers.
    pub to: Mask,
}

/// A compiled path rule.
#[derive(Debug, Clone)]
pub struct Rule {
    /// Human-readable label for warnings and `explain`.
    pub label: String,
    /// Source globs.
    pub patterns: Vec<String>,
    /// Routing override (`Some(0)` excludes the node everywhere).
    pub to: Option<Mask>,
    /// Node name override.
    pub rename: Option<String>,
    /// `$properties` to attach.
    pub set: Option<Map<String, Value>>,
    /// `$className` override for the node.
    pub class_name: Option<String>,
    /// Number of literal path segments (tie-breaker: more literal = more specific).
    pub specificity: u32,
}

/// Extra settings for a node of the project tree, addressed by its path in
/// the tree (e.g. `HttpService` or `StarterPlayer/StarterPlayerScripts`).
#[derive(Debug, Clone)]
pub struct NodeSpec {
    /// Path segments inside `tree`.
    pub path: Vec<String>,
    /// `$className`.
    pub class_name: Option<String>,
    /// `$properties` (merged into existing ones).
    pub properties: Option<Map<String, Value>>,
    /// `$path` on disk (project-relative, as Rojo reads it).
    pub fs_path: Option<String>,
    /// `$ignoreUnknownInstances`.
    pub ignore_unknown_instances: Option<bool>,
}

/// Where a routing decision came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// A rule matched the node itself.
    Rule(usize),
    /// A suffix rule matched the node name.
    Suffix(usize),
    /// Inherited from an ancestor directory rule.
    Inherited,
    /// `default_to`.
    Default,
}

/// Rules that matched a node's own path, already layered.
#[derive(Debug, Default)]
pub struct Direct {
    /// Matching rule indices, least to most specific.
    pub rules: Vec<usize>,
    /// Effective routing override and the rule that set it.
    pub to: Option<(Mask, usize)>,
    /// Effective rename.
    pub rename: Option<String>,
    /// Merged `$properties`.
    pub set: Option<Map<String, Value>>,
    /// Effective `$className` override.
    pub class_name: Option<String>,
}

/// Compiled `octatree.toml`.
#[derive(Debug)]
pub struct Settings {
    /// Project name in the generated file.
    pub project_name: String,
    /// Source directory (posix, project-relative).
    pub source_dir: String,
    /// Default output file (project-relative).
    pub output: String,
    /// Top-level module folders, in output order.
    pub modules: Vec<String>,
    /// File extensions treated as sources, longest first (with leading dot).
    pub extensions: Vec<String>,
    /// File names that turn a folder into a single node.
    pub init_files: Vec<String>,
    /// Containers in declaration order (index = bit position in [`Mask`]).
    pub containers: Vec<Container>,
    /// Routing when nothing else matches.
    pub default_to: Mask,
    /// Suffix routing, in order.
    pub suffix_rules: Vec<SuffixRule>,
    /// Path rules, in file order.
    pub rules: Vec<Rule>,
    /// Naming configuration.
    pub naming: NamingRules,
    /// Collision policy.
    pub on_collision: OnCollision,
    /// User-supplied tree template (contents of `tree`), if any.
    pub template: Option<Map<String, Value>>,
    /// True when containers are exactly the built-in two (enables the built-in scaffold).
    pub default_containers: bool,
    /// Per-node class/property/path settings applied to the tree before mounting.
    pub nodes: Vec<NodeSpec>,
    /// Extra top-level project fields (e.g. `globIgnorePaths`), written after `name`.
    pub rojo: Map<String, Value>,
    rule_globs: GlobSet,
    glob_rule: Vec<usize>,
    ignore: GlobSet,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

impl OneOrMany {
    fn into_vec(self) -> Vec<String> {
        match self {
            Self::One(s) => vec![s],
            Self::Many(v) => v,
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawContainer {
    mount: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSuffix {
    suffix: OneOrMany,
    to: Vec<String>,
    case_insensitive: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRule {
    name: Option<String>,
    #[serde(rename = "match")]
    pattern: OneOrMany,
    to: Option<Vec<String>>,
    rename: Option<String>,
    set: Option<toml::Table>,
    class_name: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawNode {
    class_name: Option<String>,
    properties: Option<toml::Table>,
    path: Option<String>,
    ignore_unknown_instances: Option<bool>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RawNaming {
    case: Option<CaseStyle>,
    keep_prefixes: Option<Vec<String>>,
    prefix_match: Option<PrefixMatch>,
    short_word_max: Option<usize>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSettings {
    version: Option<u32>,
    project_name: Option<String>,
    source_dir: Option<String>,
    output: Option<String>,
    modules: Option<Vec<String>>,
    extensions: Option<Vec<String>>,
    init_files: Option<Vec<String>>,
    default_to: Option<Vec<String>>,
    on_collision: Option<OnCollision>,
    ignore: Option<Vec<String>>,
    containers: Option<toml::Table>,
    suffix_rules: Option<Vec<RawSuffix>>,
    naming: Option<RawNaming>,
    #[serde(default)]
    rules: Vec<RawRule>,
    template: Option<toml::Table>,
    template_file: Option<String>,
    nodes: Option<toml::Table>,
    rojo: Option<toml::Table>,
}

fn normalize_path(p: &str) -> String {
    p.replace('\\', "/").trim_matches('/').to_string()
}

fn count_literal_segments(pattern: &str) -> u32 {
    let literal = pattern
        .split('/')
        .filter(|s| !s.is_empty() && !s.contains(['*', '?', '[', ']', '{', '}']))
        .count();
    u32::try_from(literal).unwrap_or(u32::MAX)
}

fn build_glob(pattern: &str) -> std::result::Result<globset::Glob, globset::Error> {
    GlobBuilder::new(&pattern.replace('\\', "/"))
        .literal_separator(true)
        .build()
}

fn toml_to_json_map(t: &toml::Table) -> std::result::Result<Map<String, Value>, String> {
    match serde_json::to_value(t).map_err(|e| e.to_string())? {
        Value::Object(m) => Ok(m),
        _ => Err("expected a table".into()),
    }
}

impl Settings {
    /// Reads and compiles a TOML settings file.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_toml(&text, path)
    }

    /// Compiles settings from TOML text. `origin` anchors `template_file` and error messages.
    pub fn from_toml(text: &str, origin: &Path) -> Result<Self> {
        let err = |message: String| Error::Settings {
            path: origin.to_path_buf(),
            message,
        };
        let raw: RawSettings = toml::from_str(text).map_err(|e| err(e.to_string()))?;

        if let Some(v) = raw.version
            && v != 1
        {
            return Err(err(format!(
                "unsupported version {v} (this build reads version 1)"
            )));
        }

        // Containers.
        let mut containers = Vec::new();
        match raw.containers {
            Some(table) if !table.is_empty() => {
                for (name, value) in table {
                    let c: RawContainer = value
                        .try_into()
                        .map_err(|e: toml::de::Error| err(format!("containers.{name}: {e}")))?;
                    let mount: Vec<String> = normalize_path(&c.mount)
                        .split('/')
                        .filter(|s| !s.is_empty())
                        .map(String::from)
                        .collect();
                    if mount.is_empty() {
                        return Err(err(format!("containers.{name}: `mount` must not be empty")));
                    }
                    containers.push(Container { name, mount });
                }
            }
            Some(_) => return Err(err("`containers` must not be empty".into())),
            None => {
                for (name, mount) in DEFAULT_MOUNTS {
                    containers.push(Container {
                        name: name.into(),
                        mount: mount.split('/').map(String::from).collect(),
                    });
                }
            }
        }
        if containers.len() > MAX_CONTAINERS {
            return Err(err(format!(
                "at most {MAX_CONTAINERS} containers are supported"
            )));
        }
        let default_containers = containers.len() == DEFAULT_MOUNTS.len()
            && containers
                .iter()
                .zip(DEFAULT_MOUNTS)
                .all(|(c, (n, m))| c.name == n && c.mount.join("/") == m);

        let mask_of = |names: &[String], what: &str| -> Result<Mask> {
            let mut mask: Mask = 0;
            for n in names {
                let Some(i) = containers.iter().position(|c| &c.name == n) else {
                    let known: Vec<&str> = containers.iter().map(|c| c.name.as_str()).collect();
                    return Err(err(format!(
                        "{what}: unknown container `{n}` (known: {})",
                        known.join(", ")
                    )));
                };
                mask |= 1 << i;
            }
            Ok(mask)
        };

        let default_to = match &raw.default_to {
            Some(names) => mask_of(names, "default_to")?,
            None if containers.iter().any(|c| c.name == "shared") => {
                mask_of(&["shared".to_string()], "default_to")?
            }
            None => 1,
        };

        // Suffix rules.
        let suffix_rules = match raw.suffix_rules {
            Some(list) => list
                .into_iter()
                .enumerate()
                .map(|(i, s)| {
                    let ci = s.case_insensitive.unwrap_or(true);
                    let suffixes = s
                        .suffix
                        .into_vec()
                        .into_iter()
                        .map(|x| if ci { x.to_lowercase() } else { x })
                        .collect();
                    Ok(SuffixRule {
                        suffixes,
                        case_insensitive: ci,
                        to: mask_of(&s.to, &format!("suffix_rules[{i}].to"))?,
                    })
                })
                .collect::<Result<Vec<_>>>()?,
            // No suffix routing unless the developer asks for it.
            None => Vec::new(),
        };

        // Path rules.
        let mut rules = Vec::with_capacity(raw.rules.len());
        let mut glob_builder = GlobSetBuilder::new();
        let mut glob_rule = Vec::new();
        for (i, r) in raw.rules.into_iter().enumerate() {
            let patterns = r.pattern.into_vec();
            let label = r
                .name
                .clone()
                .unwrap_or_else(|| format!("rules[{i}] match = {}", patterns.join(", ")));
            if patterns.is_empty() {
                return Err(err(format!("{label}: `match` must not be empty")));
            }
            if r.to.is_none() && r.rename.is_none() && r.set.is_none() && r.class_name.is_none() {
                return Err(err(format!(
                    "{label}: rule has no effect; set `to`, `rename`, `set` or `class_name`"
                )));
            }
            if r.rename.as_deref() == Some("") {
                return Err(err(format!("{label}: `rename` must not be empty")));
            }
            if r.class_name.as_deref() == Some("") {
                return Err(err(format!("{label}: `class_name` must not be empty")));
            }
            let to = match &r.to {
                Some(names) => Some(mask_of(names, &format!("{label}: to"))?),
                None => None,
            };
            let set = match &r.set {
                Some(t) => {
                    Some(toml_to_json_map(t).map_err(|e| err(format!("{label}: set: {e}")))?)
                }
                None => None,
            };
            let mut specificity = 0;
            for p in &patterns {
                let glob =
                    build_glob(p).map_err(|e| err(format!("{label}: bad pattern `{p}`: {e}")))?;
                glob_builder.add(glob);
                glob_rule.push(i);
                specificity = specificity.max(count_literal_segments(p));
            }
            rules.push(Rule {
                label,
                patterns,
                to,
                rename: r.rename,
                set,
                class_name: r.class_name,
                specificity,
            });
        }
        let rule_globs = glob_builder.build().map_err(|e| err(e.to_string()))?;

        // Ignore globs.
        let mut ignore_builder = GlobSetBuilder::new();
        for p in raw.ignore.unwrap_or_default() {
            ignore_builder
                .add(build_glob(&p).map_err(|e| err(format!("ignore: bad pattern `{p}`: {e}")))?);
        }
        let ignore = ignore_builder.build().map_err(|e| err(e.to_string()))?;

        // Extensions & init files.
        let mut extensions: Vec<String> = raw
            .extensions
            .unwrap_or_else(|| vec![".luau".into()])
            .into_iter()
            .map(|e| {
                if e.starts_with('.') {
                    e
                } else {
                    format!(".{e}")
                }
            })
            .collect();
        if extensions.is_empty() {
            return Err(err("`extensions` must not be empty".into()));
        }
        extensions.sort_by_key(|e| std::cmp::Reverse(e.len()));
        let init_files = raw.init_files.unwrap_or_else(|| {
            ["init.luau", "init.server.luau", "init.client.luau"]
                .map(String::from)
                .into()
        });

        // Naming.
        let rn = raw.naming.unwrap_or_default();
        let naming = NamingRules {
            case: rn.case.unwrap_or_default(),
            keep_prefixes: rn.keep_prefixes.unwrap_or_default(),
            prefix_match: rn.prefix_match.unwrap_or_default(),
            short_word_max: rn.short_word_max.unwrap_or(2),
        };

        // Modules.
        let modules = raw.modules.unwrap_or_default();
        for (i, m) in modules.iter().enumerate() {
            if m.is_empty() || m.contains(['/', '\\']) {
                return Err(err(format!(
                    "modules[{i}] `{m}` must be a single folder name"
                )));
            }
            if modules[..i].contains(m) {
                return Err(err(format!("module `{m}` is listed twice")));
            }
        }

        // Template.
        let template = match (raw.template, raw.template_file) {
            (Some(_), Some(_)) => {
                return Err(err(
                    "use either `template` or `template_file`, not both".into()
                ));
            }
            (Some(t), None) => {
                Some(toml_to_json_map(&t).map_err(|e| err(format!("template: {e}")))?)
            }
            (None, Some(file)) => {
                let full = origin
                    .parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(&file);
                let text = std::fs::read_to_string(&full).map_err(|source| Error::Io {
                    path: full.clone(),
                    source,
                })?;
                match serde_json::from_str::<Value>(&text) {
                    Ok(Value::Object(m)) => Some(m),
                    Ok(_) => {
                        return Err(err(format!("template_file {file}: expected a JSON object")));
                    }
                    Err(e) => return Err(err(format!("template_file {file}: {e}"))),
                }
            }
            (None, None) => None,
        };

        // Per-node settings, addressed by path inside the tree.
        let mut nodes = Vec::new();
        for (key, value) in raw.nodes.unwrap_or_default() {
            let n: RawNode = value
                .try_into()
                .map_err(|e: toml::de::Error| err(format!("nodes.\"{key}\": {e}")))?;
            let path: Vec<String> = normalize_path(&key)
                .split('/')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect();
            if path.is_empty() {
                return Err(err(
                    "a `nodes` key must be a path such as \"HttpService\"".into()
                ));
            }
            if n.class_name.is_none()
                && n.properties.is_none()
                && n.path.is_none()
                && n.ignore_unknown_instances.is_none()
            {
                return Err(err(format!(
                    "nodes.\"{key}\": set at least one of `class_name`, `properties`, `path`, `ignore_unknown_instances`"
                )));
            }
            if n.class_name.as_deref() == Some("") {
                return Err(err(format!(
                    "nodes.\"{key}\": `class_name` must not be empty"
                )));
            }
            let properties = match &n.properties {
                Some(t) => Some(
                    toml_to_json_map(t)
                        .map_err(|e| err(format!("nodes.\"{key}\": properties: {e}")))?,
                ),
                None => None,
            };
            nodes.push(NodeSpec {
                path,
                class_name: n.class_name,
                properties,
                fs_path: n.path,
                ignore_unknown_instances: n.ignore_unknown_instances,
            });
        }

        // Extra top-level project fields (globIgnorePaths, placeId, ...).
        let rojo = match &raw.rojo {
            Some(t) => {
                if t.contains_key("name") || t.contains_key("tree") {
                    return Err(err(
                        "`[rojo]` cannot set `name` (use `project_name`) or `tree`".into(),
                    ));
                }
                toml_to_json_map(t).map_err(|e| err(format!("rojo: {e}")))?
            }
            None => Map::new(),
        };

        let source_dir = raw
            .source_dir
            .map_or_else(|| "src".to_string(), |s| normalize_path(&s));
        if source_dir.is_empty() {
            return Err(err("`source_dir` must not be empty".into()));
        }

        Ok(Self {
            project_name: raw.project_name.unwrap_or_else(|| "Template".into()),
            source_dir,
            output: raw.output.unwrap_or_else(|| "default.project.json".into()),
            modules,
            extensions,
            init_files,
            containers,
            default_to,
            suffix_rules,
            rules,
            naming,
            on_collision: raw.on_collision.unwrap_or_default(),
            template,
            default_containers,
            nodes,
            rojo,
            rule_globs,
            glob_rule,
            ignore,
        })
    }

    /// True when `path` is excluded by an `ignore` glob.
    pub fn is_ignored(&self, path: &str) -> bool {
        !self.ignore.is_empty() && self.ignore.is_match(path)
    }

    /// Strips a configured extension; `None` when `name` is not a source file.
    pub fn strip_extension<'a>(&self, name: &'a str) -> Option<&'a str> {
        self.extensions
            .iter()
            .find_map(|e| name.strip_suffix(e.as_str()))
            .filter(|stem| !stem.is_empty())
    }

    /// Whether `name` is one of the `init_files`.
    pub fn is_init_file(&self, name: &str) -> bool {
        self.init_files.iter().any(|f| f == name)
    }

    /// Layers every rule matching `path` itself (least to most specific).
    /// `buf` is scratch space reused across calls.
    pub fn direct(&self, path: &str, buf: &mut Vec<usize>) -> Direct {
        let mut d = Direct::default();
        if self.rules.is_empty() {
            return d;
        }
        buf.clear();
        self.rule_globs.matches_into(path, buf);
        if buf.is_empty() {
            return d;
        }
        let mut rules: Vec<usize> = buf.iter().map(|&g| self.glob_rule[g]).collect();
        rules.sort_by_key(|&r| (self.rules[r].specificity, r));
        rules.dedup();
        for &r in &rules {
            let rule = &self.rules[r];
            if let Some(mask) = rule.to {
                d.to = Some((mask, r));
            }
            if let Some(name) = &rule.rename {
                d.rename = Some(name.clone());
            }
            if let Some(class) = &rule.class_name {
                d.class_name = Some(class.clone());
            }
            if let Some(set) = &rule.set {
                let merged = d.set.get_or_insert_with(Map::new);
                for (k, v) in set {
                    merged.insert(k.clone(), v.clone());
                }
            }
        }
        d.rules = rules;
        d
    }

    /// Index of the first suffix rule matching `name` (extension already stripped).
    pub fn suffix_match(&self, name: &str) -> Option<usize> {
        let lower = name.to_lowercase();
        self.suffix_rules.iter().position(|r| {
            r.suffixes.iter().any(|s| {
                if r.case_insensitive {
                    lower.ends_with(s.as_str())
                } else {
                    name.ends_with(s.as_str())
                }
            })
        })
    }

    /// Routing decision for a leaf node: rule > suffix > inherited > default.
    pub fn decide(&self, stem: &str, d: &Direct, inherited: Option<Mask>) -> (Mask, Source) {
        if let Some((mask, rule)) = d.to {
            return (mask, Source::Rule(rule));
        }
        if let Some(i) = self.suffix_match(stem) {
            return (self.suffix_rules[i].to, Source::Suffix(i));
        }
        match inherited {
            Some(mask) => (mask, Source::Inherited),
            None => (self.default_to, Source::Default),
        }
    }

    /// Final node name: a `rename` wins, otherwise the naming rules apply.
    pub fn node_name(&self, stem: &str, d: &Direct) -> String {
        d.rename.clone().unwrap_or_else(|| self.naming.apply(stem))
    }

    /// Container names in `mask`, in declaration order.
    pub fn mask_names(&self, mask: Mask) -> Vec<&str> {
        self.containers
            .iter()
            .enumerate()
            .filter(|(i, _)| mask >> i & 1 == 1)
            .map(|(_, c)| c.name.as_str())
            .collect()
    }
}

/// A commented starter config written by `octatree init`. It describes no
/// project structure: nothing is scanned until `modules` is filled in.
pub const STARTER_TOML: &str = r##"# OctaTree configuration.
# Run `octatree explain <path>` to see why a file lands where it does.
version = 1

project_name = "Template"
source_dir = "src"
output = "default.project.json"

# Folders under source_dir that are scanned, in output order.
# Nothing is scanned until you list them, for example: modules = ["my_folder"]
modules = []

# File types treated as sources (anchored name endings).
extensions = [".luau"]

# Containers that receive files no rule decides.
default_to = ["shared"]

# What to do when two entries get the same node name: "warn", "error" or "last-wins".
on_collision = "warn"

# Paths that are never scanned (globs, project-relative, forward slashes).
ignore = []

# Containers are the places in the tree where your files are mounted.
# Add more as needed, for example:
# [containers.client]
# mount = "StarterPlayer/StarterPlayerScripts"
[containers.shared]
mount = "ReplicatedStorage"

[containers.server]
mount = "ServerScriptService"

# Route files by the end of their name, for example:
# [[suffix_rules]]
# suffix = ["server"]
# to = ["server"]

[naming]
case = "pascal"        # pascal | camel | snake | keep
keep_prefixes = []     # names starting with one of these keep their casing
prefix_match = "word"  # word | starts_with
short_word_max = 2     # words this short become ALL CAPS; 0 turns it off

# Rules route, rename or tag the nodes whose own path matches `match`
# (a glob). A rule on a folder applies to everything below it. A more
# specific rule (more literal path segments), then a later rule, wins.
# A rule on a file beats a suffix rule, which beats a folder rule.
#
# [[rules]]
# match = "src/my_folder/my_subfolder"
# to = ["server"]
# rename = "MyName"             # optional: node name
# class_name = "Folder"         # optional: $className of the node
# set = { Key = "Value" }       # optional: $properties of the node

# Class, properties or path for nodes of the tree that are not your files
# (services, parents of a mount). Keys are paths inside the tree.
# [nodes.SomeService]
# class_name = "SomeService"
# properties = { Key = "Value" }

# Extra top-level fields of the project file, written after `name`.
# [rojo]
# globIgnorePaths = ["**/*.spec.lua"]

# Optional: your own tree skeleton (the contents of `tree`). Without one,
# the tree starts with ReplicatedStorage and ServerScriptService, each with
# "$ignoreUnknownInstances" = true.
# [template.ReplicatedStorage]
# "$ignoreUnknownInstances" = true
# or: template_file = "template.json"
"##;

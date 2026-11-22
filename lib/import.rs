//! Reads the JSON config format of an older, similar tool so that
//! `octatree migrate` can convert it. This is import-only: generation always
//! uses `octatree.toml`.
//!
//! Recognized keys (all optional): `context-server`, `server-black-list`,
//! `duplicated-modules` and `dontUsePascalCase.prefixes`.

use std::path::Path;

use serde::{Deserialize, Deserializer};

use crate::error::{Error, Result};

/// Treats JSON `null` like a missing key.
fn null_default<'de, D, T>(d: D) -> std::result::Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: Default + Deserialize<'de>,
{
    Ok(Option::<T>::deserialize(d)?.unwrap_or_default())
}

#[derive(Debug, Deserialize)]
struct ContextServerEntry {
    #[serde(rename = "$path")]
    path: String,
    #[serde(rename = "$name", default, deserialize_with = "null_default")]
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PathEntry {
    #[serde(rename = "$path")]
    path: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum DuplicatedEntry {
    /// A bare string `s` means the folder `src/s`.
    Bare(String),
    Object {
        #[serde(rename = "$path")]
        path: Option<String>,
    },
    /// Anything else is ignored.
    Other(#[allow(dead_code)] serde::de::IgnoredAny),
}

#[derive(Debug, Default, Deserialize)]
struct NamingSection {
    #[serde(default, deserialize_with = "null_default")]
    prefixes: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
struct RawImport {
    #[serde(rename = "context-server", default, deserialize_with = "null_default")]
    context_server: Vec<ContextServerEntry>,
    #[serde(
        rename = "server-black-list",
        default,
        deserialize_with = "null_default"
    )]
    server_black_list: Vec<PathEntry>,
    #[serde(
        rename = "duplicated-modules",
        default,
        deserialize_with = "null_default"
    )]
    duplicated_modules: Vec<DuplicatedEntry>,
    #[serde(
        rename = "dontUsePascalCase",
        default,
        deserialize_with = "null_default"
    )]
    dont_use_pascal_case: NamingSection,
    /// Unrecognized keys, reported so nothing is dropped silently.
    #[serde(flatten)]
    extra: std::collections::BTreeMap<String, serde_json::Value>,
}

/// The routing information found in an old config, in file order.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Imported {
    /// Server-only paths, with an optional custom node name.
    pub server_only: Vec<(String, Option<String>)>,
    /// Paths that must not be mounted on the server.
    pub no_server: Vec<String>,
    /// Paths mounted in both shared and server.
    pub duplicated: Vec<String>,
    /// Names starting with one of these keep their casing.
    pub keep_prefixes: Vec<String>,
    /// Keys that were not recognized.
    pub unknown_keys: Vec<String>,
}

fn to_posix(p: &str) -> String {
    p.replace('\\', "/")
}

impl Imported {
    /// Parses the JSON text. `origin` is only used in error messages.
    pub fn from_json(text: &str, origin: &Path) -> Result<Self> {
        let raw: RawImport = serde_json::from_str(text).map_err(|source| Error::Config {
            path: origin.to_path_buf(),
            source,
        })?;
        Ok(Self {
            server_only: raw
                .context_server
                .into_iter()
                // An empty name counts as absent.
                .map(|e| (to_posix(&e.path), e.name.filter(|n| !n.is_empty())))
                .collect(),
            no_server: raw
                .server_black_list
                .into_iter()
                .map(|e| to_posix(&e.path))
                .collect(),
            duplicated: raw
                .duplicated_modules
                .into_iter()
                .filter_map(|e| match e {
                    DuplicatedEntry::Bare(s) => Some(format!("src/{s}")),
                    DuplicatedEntry::Object { path } => path.map(|p| to_posix(&p)),
                    DuplicatedEntry::Other(_) => None,
                })
                .collect(),
            keep_prefixes: raw.dont_use_pascal_case.prefixes,
            unknown_keys: raw.extra.into_keys().collect(),
        })
    }

    /// Reads and parses a file.
    pub fn load(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path).map_err(|source| Error::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Self::from_json(&text, path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_sections() {
        let json = r#"{
            "context-server": [{"$path": "src/a"}, {"$path": "src\\b", "$name": "B"}, {"$path": "src/c", "$name": ""}],
            "server-black-list": [{"$path": "src/ui"}],
            "dontUsePascalCase": {"prefixes": ["use"]},
            "duplicated-modules": ["x", {"$path": "src/y"}, {"nope": 1}, 7],
            "typo-key": []
        }"#;
        let c = Imported::from_json(json, Path::new("t.json")).unwrap();
        assert_eq!(
            c.server_only,
            [
                ("src/a".to_string(), None),
                ("src/b".to_string(), Some("B".to_string())),
                ("src/c".to_string(), None)
            ]
        );
        assert_eq!(c.no_server, ["src/ui"]);
        assert_eq!(c.duplicated, ["src/x", "src/y"]);
        assert_eq!(c.keep_prefixes, ["use"]);
        assert_eq!(c.unknown_keys, ["typo-key"]);
    }

    #[test]
    fn nulls_and_empty_documents_are_fine() {
        let c = Imported::from_json(
            r#"{"context-server": null, "dontUsePascalCase": null}"#,
            Path::new("t"),
        )
        .unwrap();
        assert_eq!(c, Imported::default());
        assert_eq!(
            Imported::from_json("{}", Path::new("t")).unwrap(),
            Imported::default()
        );
    }

    #[test]
    fn missing_path_is_an_error() {
        assert!(Imported::from_json(r#"{"server-black-list": [{}]}"#, Path::new("t")).is_err());
    }
}

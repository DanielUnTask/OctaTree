//! OctaTree: generates a Rojo `default.project.json` from a Luau source tree,
//! driven by an `octatree.toml`.
//!
//! Pipeline: [`scan`] (I/O) → [`settings`] (pure routing and naming decisions)
//! → [`engine`] (tree building) → [`emit`] (I/O).

pub mod emit;
pub mod engine;
pub mod error;
pub mod explain;
pub mod import;
pub mod migrate;
pub mod naming;
pub mod scan;
pub mod settings;
pub mod watch;

use error::Result;
use scan::DirReader;
use settings::Settings;

/// Output of [`generate`].
#[derive(Debug)]
pub struct Generated {
    /// Pretty-printed project JSON (2-space indent, no trailing newline).
    pub json: String,
    /// Non-fatal problems to show the user.
    pub warnings: Vec<String>,
}

/// Builds the project JSON for `settings` from the directories exposed by `reader`.
pub fn generate<R: DirReader>(settings: &Settings, reader: &mut R) -> Result<Generated> {
    let mut built = engine::build(settings, reader)?;
    built.warnings.extend(reader.take_warnings());
    let json = serde_json::to_string_pretty(&built.tree)?;
    Ok(Generated {
        json,
        warnings: built.warnings,
    })
}

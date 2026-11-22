//! Output stage: write the generated file only when its bytes change.

use std::path::Path;

use crate::error::{Error, Result};

/// True when `path` already contains exactly `content`.
pub fn is_up_to_date(path: &Path, content: &str) -> Result<bool> {
    match std::fs::read(path) {
        Ok(existing) => Ok(existing == content.as_bytes()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(Error::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

/// Writes `content` unless the file is already identical (keeps mtime stable
/// so file watchers don't churn). Returns whether a write happened.
pub fn write_if_changed(path: &Path, content: &str) -> Result<bool> {
    if is_up_to_date(path, content)? {
        return Ok(false);
    }
    std::fs::write(path, content).map_err(|source| Error::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(true)
}

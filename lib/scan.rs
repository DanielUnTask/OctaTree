//! Filesystem access. All directory I/O goes through [`DirReader`], so the
//! rest of the generator can be tested against an in-memory tree.

use std::cmp::Ordering;
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::error::{Error, Result};

/// One directory entry (name only, no path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// File or directory name.
    pub name: String,
    /// True for real directories (symlinks are not followed, like Node's `Dirent`).
    pub is_dir: bool,
}

/// Source of directory listings. Paths are project-relative with `/` separators.
pub trait DirReader {
    /// Lists `rel`. Returns `Ok(None)` when the directory does not exist.
    fn read_dir(&mut self, rel: &str) -> Result<Option<Vec<Entry>>>;

    /// Non-fatal problems found while reading (drained by the caller).
    fn take_warnings(&mut self) -> Vec<String> {
        Vec::new()
    }
}

/// Deterministic entry order on every OS: case-insensitive ordinal (the
/// same order NTFS lists files in), then byte-wise.
pub fn compare_names(a: &str, b: &str) -> Ordering {
    a.chars()
        .flat_map(char::to_uppercase)
        .cmp(b.chars().flat_map(char::to_uppercase))
        .then_with(|| a.cmp(b))
}

/// Reads the real filesystem under `root`.
#[derive(Debug)]
pub struct FsReader {
    root: PathBuf,
    warnings: Vec<String>,
}

impl FsReader {
    /// Creates a reader rooted at the project directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            warnings: Vec::new(),
        }
    }
}

impl DirReader for FsReader {
    fn read_dir(&mut self, rel: &str) -> Result<Option<Vec<Entry>>> {
        let dir = self.root.join(rel);
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            // Missing, or a file where a directory was expected (Windows reports
            // the latter as raw error 267 / ERROR_DIRECTORY).
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::NotADirectory
                ) || e.raw_os_error() == Some(267) =>
            {
                return Ok(None);
            }
            Err(source) => return Err(Error::Io { path: dir, source }),
        };
        let mut out = Vec::new();
        for ent in rd {
            let ent = ent.map_err(|source| Error::Io {
                path: dir.clone(),
                source,
            })?;
            let file_type = ent.file_type().map_err(|source| Error::Io {
                path: ent.path(),
                source,
            })?;
            match ent.file_name().into_string() {
                Ok(name) => out.push(Entry {
                    name,
                    is_dir: file_type.is_dir(),
                }),
                Err(raw) => self
                    .warnings
                    .push(format!("skipped non-UTF-8 name {raw:?} in {rel}")),
            }
        }
        Ok(Some(out))
    }

    fn take_warnings(&mut self) -> Vec<String> {
        std::mem::take(&mut self.warnings)
    }
}

/// In-memory tree built from a list of file paths (used by tests).
/// A path ending in `/` declares an empty directory.
#[derive(Debug, Default)]
pub struct MemReader {
    dirs: BTreeMap<String, Vec<Entry>>,
}

impl MemReader {
    /// Builds the tree from project-relative paths such as `src/folder/A.luau`.
    pub fn from_paths<I, S>(paths: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut dirs: BTreeMap<String, Vec<Entry>> = BTreeMap::new();
        for p in paths {
            let p = p.as_ref().trim();
            if p.is_empty() {
                continue;
            }
            let ends_dir = p.ends_with('/');
            let segs: Vec<&str> = p.trim_end_matches('/').split('/').collect();
            for i in 0..segs.len() {
                let parent = segs[..i].join("/");
                let is_dir = i + 1 < segs.len() || ends_dir;
                let list = dirs.entry(parent).or_default();
                if !list.iter().any(|e| e.name == segs[i]) {
                    list.push(Entry {
                        name: segs[i].to_string(),
                        is_dir,
                    });
                }
                if is_dir {
                    dirs.entry(segs[..=i].join("/")).or_default();
                }
            }
        }
        Self { dirs }
    }
}

impl DirReader for MemReader {
    fn read_dir(&mut self, rel: &str) -> Result<Option<Vec<Entry>>> {
        Ok(self.dirs.get(rel).cloned())
    }
}

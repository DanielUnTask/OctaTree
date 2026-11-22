//! `octatree watch` and `octatree stop`.
//!
//! - [`Watcher`] regenerates the project file whenever the result changes: on
//!   every [`Watcher::tick`] it re-reads the configuration (a broken edit keeps
//!   the previous one) and regenerates in memory; the file is only written when
//!   the output differs. Polling needs no extra dependency and behaves the
//!   same on every OS and file system.
//! - [`Control`] lets a second process stop a running watcher without signals
//!   or process ids: the watcher refreshes a *heartbeat* file in the temp
//!   directory, and `octatree stop` drops a *stop* marker the watcher notices.
//!   A watcher that dies leaves a heartbeat that goes stale, so nothing stays
//!   blocked.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use crate::emit;
use crate::error::{Error, Result};
use crate::generate;
use crate::scan::FsReader;
use crate::settings::Settings;

/// A heartbeat older than this means the watcher is gone.
const HEARTBEAT_STALE: Duration = Duration::from_secs(5);

fn io_err(path: &Path, source: std::io::Error) -> Error {
    Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// FNV-1a: a stable hash (unlike `DefaultHasher`) so two processes agree on
/// the control file names.
fn fnv1a(s: &str) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in s.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    h
}

/// The two marker files shared by `watch` and `stop`.
#[derive(Debug, Clone)]
pub struct Control {
    heartbeat: PathBuf,
    stop: PathBuf,
}

impl Control {
    /// Control files inside `dir`, named after `id`.
    pub fn in_dir(dir: &Path, id: &str) -> Self {
        Self {
            heartbeat: dir.join(format!("octatree-watch-{id}.lock")),
            stop: dir.join(format!("octatree-watch-{id}.stop")),
        }
    }

    /// Control files for the project at `root` (in the OS temp directory, so
    /// nothing is written into the project).
    pub fn for_root(root: &Path) -> Self {
        let canonical = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
        let id = format!("{:016x}", fnv1a(&canonical.to_string_lossy()));
        Self::in_dir(&std::env::temp_dir(), &id)
    }

    /// True when a watcher refreshed its heartbeat recently.
    pub fn is_active(&self) -> bool {
        fs::metadata(&self.heartbeat)
            .and_then(|m| m.modified())
            .is_ok_and(|t| {
                SystemTime::now()
                    .duration_since(t)
                    .map_or(true, |age| age < HEARTBEAT_STALE)
            })
    }

    /// Refreshes the heartbeat (called by the watcher about once a second).
    pub fn beat(&self) -> Result<()> {
        let content = format!("pid={}\n", std::process::id());
        fs::write(&self.heartbeat, content).map_err(|e| io_err(&self.heartbeat, e))
    }

    /// Asks the running watcher to stop.
    pub fn request_stop(&self) -> Result<()> {
        fs::write(&self.stop, b"stop").map_err(|e| io_err(&self.stop, e))
    }

    /// True once `octatree stop` has been run.
    pub fn stop_requested(&self) -> bool {
        self.stop.exists()
    }

    /// Marks a watcher as running. The returned guard removes the marker
    /// files when dropped, including on early returns and panics.
    pub fn start(&self) -> Result<WatchGuard<'_>> {
        let _ = fs::remove_file(&self.stop);
        self.beat()?;
        Ok(WatchGuard(self))
    }

    fn clear(&self) {
        let _ = fs::remove_file(&self.heartbeat);
        let _ = fs::remove_file(&self.stop);
    }
}

/// Removes the control files when the watcher ends.
#[derive(Debug)]
pub struct WatchGuard<'a>(&'a Control);

impl Drop for WatchGuard<'_> {
    fn drop(&mut self) {
        self.0.clear();
    }
}

/// Regenerates the project file when the result changes.
#[derive(Debug)]
pub struct Watcher {
    root: PathBuf,
    config_path: PathBuf,
    out_override: Option<PathBuf>,
    settings: Settings,
    config_text: String,
    last_json: Option<String>,
    last_warnings: Vec<String>,
    last_error: Option<String>,
}

impl Watcher {
    /// Loads the configuration; fails if it is missing or invalid.
    pub fn new(root: PathBuf, config_path: PathBuf, out_override: Option<PathBuf>) -> Result<Self> {
        let config_text = fs::read_to_string(&config_path).map_err(|e| io_err(&config_path, e))?;
        let settings = Settings::from_toml(&config_text, &config_path)?;
        Ok(Self {
            root,
            config_path,
            out_override,
            settings,
            config_text,
            last_json: None,
            last_warnings: Vec::new(),
            last_error: None,
        })
    }

    /// The file being written.
    pub fn output_path(&self) -> PathBuf {
        self.out_override
            .clone()
            .unwrap_or_else(|| self.root.join(&self.settings.output))
    }

    fn report_error(&mut self, log: &mut dyn FnMut(String), message: String) {
        if self.last_error.as_deref() != Some(message.as_str()) {
            log(format!("error: {message}"));
            self.last_error = Some(message);
        }
    }

    /// One check: reload the config if it changed, regenerate, and write the
    /// file if the output changed. Messages go to `log`; the same problem is
    /// reported once, and nothing is logged when nothing changed.
    pub fn tick(&mut self, log: &mut dyn FnMut(String)) {
        match fs::read_to_string(&self.config_path) {
            Ok(text) if text != self.config_text => {
                // Remember the text even if it is invalid, so a broken edit
                // is reported once instead of on every tick.
                self.config_text.clone_from(&text);
                match Settings::from_toml(&text, &self.config_path) {
                    Ok(settings) => {
                        self.settings = settings;
                        self.last_json = None;
                        self.last_error = None;
                        log("configuration reloaded".into());
                    }
                    Err(e) => {
                        self.report_error(log, format!("{e} (keeping the previous configuration)"));
                    }
                }
            }
            Ok(_) => {}
            Err(e) => {
                let message = format!("cannot read {}: {e}", self.config_path.display());
                self.report_error(log, message);
                return;
            }
        }

        let mut reader = FsReader::new(self.root.clone());
        let generated = match generate(&self.settings, &mut reader) {
            Ok(g) => g,
            Err(e) => {
                self.report_error(log, e.to_string());
                return;
            }
        };
        self.last_error = None;

        let json_changed = self.last_json.as_deref() != Some(generated.json.as_str());
        let warnings_changed = generated.warnings != self.last_warnings;
        if !json_changed && !warnings_changed {
            return;
        }

        if json_changed {
            let out = self.output_path();
            match emit::write_if_changed(&out, &generated.json) {
                Ok(true) => log(format!("generated {}", out.display())),
                Ok(false) => log(format!("{} is up to date", out.display())),
                Err(e) => {
                    self.report_error(log, e.to_string());
                    return;
                }
            }
        }
        if warnings_changed {
            for w in &generated.warnings {
                log(format!("warning: {w}"));
            }
        }
        self.last_json = Some(generated.json);
        self.last_warnings = generated.warnings;
    }
}

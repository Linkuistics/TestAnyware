//! The app's evaluation of `config.rhai` (ADR-0021, spec "Configuration").
//!
//! The app evaluates the file once, when it starts, and a restart reloads it. In the app the
//! file beats the built-in default, and nothing else is in the order. A file that fails gives the
//! built-in defaults and the error, which the menu and `GET /v1/status` show.
//!
//! Everything here is a pure function of its arguments. `read`, `stat` and the clock are
//! parameters, so a test needs no file system.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use testanyware_app_protocol::{AppError, ConfigurationState, ConfigurationStatus, Directories};
use testanyware_config::{load, Config, ConfigError, ConfigLocation, Loaded};
use testanyware_vm::tools::ToolSearch;

/// What identifies one version of the file on disk. A rewrite of the file changes its length or
/// its modification time, and the app compares the two to tell a person to restart it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fingerprint {
    modified: Option<SystemTime>,
    len: u64,
}

/// The fingerprint of the file at `path`, or `None` when no file is there.
pub fn stat_file(path: &Path) -> Option<Fingerprint> {
    let metadata = std::fs::metadata(path).ok()?;
    Some(Fingerprint {
        modified: metadata.modified().ok(),
        len: metadata.len(),
    })
}

/// What the evaluation found.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Outcome {
    /// The run reads no file.
    Disabled,
    /// The file does not exist.
    Absent(PathBuf),
    Loaded {
        file: PathBuf,
        config: Config,
    },
    Failed(ConfigError),
}

/// The app's reading of `config.rhai`, fixed at the app's start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    outcome: Outcome,
    /// When the app evaluated the file. `None` when the run reads no file.
    evaluated_at: Option<SystemTime>,
    /// The file as the app saw it just before it read the text. `None` when no file was there.
    fingerprint: Option<Fingerprint>,
}

impl AppConfig {
    /// A run that reads no file, which is what a test without `--config` gets.
    pub fn disabled() -> Self {
        Self {
            outcome: Outcome::Disabled,
            evaluated_at: None,
            fingerprint: None,
        }
    }

    /// Evaluate the file at `location`. The fingerprint comes before the read, so that an edit
    /// between the two shows up as a change and not as the version the app evaluated.
    pub fn evaluate(
        location: &ConfigLocation,
        read: &dyn Fn(&Path) -> io::Result<String>,
        stat: &dyn Fn(&Path) -> Option<Fingerprint>,
        now: SystemTime,
    ) -> Self {
        let ConfigLocation::File(file) = location else {
            return Self::disabled();
        };
        let fingerprint = stat(file);
        let outcome = match load(location, read) {
            Ok(Loaded::Present { file, config }) => Outcome::Loaded { file, config },
            Ok(Loaded::Absent(file)) => Outcome::Absent(file),
            Ok(Loaded::Disabled) => Outcome::Disabled,
            Err(error) => Outcome::Failed(error),
        };
        Self {
            outcome,
            evaluated_at: Some(now),
            fingerprint,
        }
    }

    /// Evaluate the file on this host, now.
    pub fn evaluate_now(location: &ConfigLocation) -> Self {
        Self::evaluate(
            location,
            &|file| std::fs::read_to_string(file),
            &stat_file,
            SystemTime::now(),
        )
    }

    /// The file the app looked for, or `None` when the run reads none.
    pub fn file(&self) -> Option<&Path> {
        match &self.outcome {
            Outcome::Disabled => None,
            Outcome::Absent(file) | Outcome::Loaded { file, .. } => Some(file),
            Outcome::Failed(error) => Some(&error.file),
        }
    }

    /// True when a file is there to open: it loaded, or it failed to evaluate.
    pub fn has_file(&self) -> bool {
        matches!(self.outcome, Outcome::Loaded { .. } | Outcome::Failed(_))
    }

    /// The settings of a file that evaluated.
    fn settings(&self) -> Option<&Config> {
        match &self.outcome {
            Outcome::Loaded { config, .. } => Some(config),
            _ => None,
        }
    }

    /// The directories the app holds: the file's where it names them, else `default`. A file
    /// that names one directory leaves the other to the default.
    pub fn directories(&self, default: Directories) -> Directories {
        let Some(config) = self.settings() else {
            return default;
        };
        Directories {
            state_dir: config.state_dir.clone().unwrap_or(default.state_dir),
            data_dir: config.data_dir.clone().unwrap_or(default.data_dir),
        }
    }

    /// `search` with the tools the file locates. A tool the file names is found at that
    /// location or not at all, and a tool it does not name comes from `search`.
    pub fn tool_search(&self, search: ToolSearch) -> ToolSearch {
        let Some(config) = self.settings() else {
            return search;
        };
        let configured = config
            .tools
            .named()
            .into_iter()
            .map(|(name, location)| (name.to_string(), location.to_path_buf()))
            .collect();
        search.with_configured(configured)
    }

    /// The state that `GET /v1/status` and the menu report. `stat` reads the file as it is now,
    /// so the report can say that the file is no longer the one the app evaluated.
    pub fn status(&self, stat: &dyn Fn(&Path) -> Option<Fingerprint>) -> ConfigurationStatus {
        let (state, error) = match &self.outcome {
            Outcome::Disabled => (ConfigurationState::Disabled, None),
            Outcome::Absent(_) => (ConfigurationState::Absent, None),
            Outcome::Loaded { .. } => (ConfigurationState::Loaded, None),
            Outcome::Failed(error) => (
                ConfigurationState::Failed,
                Some(AppError::config_invalid(
                    &error.file,
                    error.line,
                    &error.message,
                )),
            ),
        };
        ConfigurationStatus {
            state,
            file: self.file().map(Path::to_path_buf),
            evaluated_at: self
                .evaluated_at
                .and_then(|at| at.duration_since(UNIX_EPOCH).ok())
                .map(|since| since.as_secs()),
            changed_since_evaluation: self
                .file()
                .is_some_and(|file| stat(file) != self.fingerprint),
            error,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::time::Duration;

    const FILE: &str = "/h/.config/testanyware/config.rhai";

    fn location() -> ConfigLocation {
        ConfigLocation::File(PathBuf::from(FILE))
    }

    fn fingerprint(len: u64) -> Fingerprint {
        Fingerprint {
            modified: Some(UNIX_EPOCH + Duration::from_secs(100 + len)),
            len,
        }
    }

    fn at(secs: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(secs)
    }

    fn defaults() -> Directories {
        Directories {
            state_dir: "/default/state".into(),
            data_dir: "/default/data".into(),
        }
    }

    /// A disk of one file, or none, that a test rewrites between the calls.
    struct Disk(RefCell<HashMap<PathBuf, String>>);

    impl Disk {
        fn with(source: Option<&str>) -> Self {
            let mut files = HashMap::new();
            if let Some(source) = source {
                files.insert(PathBuf::from(FILE), source.to_string());
            }
            Disk(RefCell::new(files))
        }
        fn write(&self, source: &str) {
            self.0
                .borrow_mut()
                .insert(PathBuf::from(FILE), source.into());
        }
        fn read(&self) -> impl Fn(&Path) -> io::Result<String> + '_ {
            |file| {
                self.0
                    .borrow()
                    .get(file)
                    .cloned()
                    .ok_or_else(|| io::ErrorKind::NotFound.into())
            }
        }
        fn stat(&self) -> impl Fn(&Path) -> Option<Fingerprint> + '_ {
            |file| {
                self.0
                    .borrow()
                    .get(file)
                    .map(|source| fingerprint(source.len() as u64))
            }
        }
    }

    fn evaluate(disk: &Disk) -> AppConfig {
        AppConfig::evaluate(&location(), &disk.read(), &disk.stat(), at(1_000))
    }

    #[test]
    fn a_run_without_a_file_keeps_the_defaults_and_reports_it_disabled() {
        let config = AppConfig::disabled();
        assert_eq!(config.directories(defaults()), defaults());
        let status = config.status(&|_| None);
        assert_eq!(status, ConfigurationStatus::default());
        assert!(!config.has_file());
    }

    #[test]
    fn a_file_that_names_the_directories_beats_the_defaults() {
        let disk = Disk::with(Some(r#"#{ state_dir: "/f/state", data_dir: "/f/data" }"#));
        let config = evaluate(&disk);
        assert_eq!(
            config.directories(defaults()),
            Directories {
                state_dir: "/f/state".into(),
                data_dir: "/f/data".into()
            }
        );
        let status = config.status(&disk.stat());
        assert_eq!(status.state, ConfigurationState::Loaded);
        assert_eq!(status.file.as_deref(), Some(Path::new(FILE)));
        assert_eq!(status.evaluated_at, Some(1_000));
        assert!(!status.changed_since_evaluation);
        assert!(status.error.is_none());
    }

    #[test]
    fn a_file_that_names_one_directory_leaves_the_other_to_the_default() {
        let disk = Disk::with(Some(r#"#{ data_dir: "/f/data" }"#));
        let directories = evaluate(&disk).directories(defaults());
        assert_eq!(directories.state_dir, PathBuf::from("/default/state"));
        assert_eq!(directories.data_dir, PathBuf::from("/f/data"));
    }

    #[test]
    fn a_file_that_fails_gives_the_defaults_and_the_config_invalid_object() {
        let disk = Disk::with(Some("let x = ;"));
        let config = evaluate(&disk);
        assert_eq!(config.directories(defaults()), defaults());
        assert!(config.has_file());
        let status = config.status(&disk.stat());
        assert_eq!(status.state, ConfigurationState::Failed);
        assert_eq!(status.file.as_deref(), Some(Path::new(FILE)));
        let error = status.error.expect("a failed file carries its error");
        assert_eq!(error.code, "CONFIG_INVALID");
        assert_eq!(error.details["file"], FILE);
        assert_eq!(error.details["line"], 1);
    }

    #[test]
    fn an_unknown_key_fails_with_no_line() {
        let disk = Disk::with(Some(r#"#{ state_directory: "/f" }"#));
        let status = evaluate(&disk).status(&disk.stat());
        assert_eq!(status.state, ConfigurationState::Failed);
        assert_eq!(
            status.error.unwrap().details["line"],
            serde_json::Value::Null
        );
    }

    #[test]
    fn a_file_that_does_not_exist_is_no_error() {
        let disk = Disk::with(None);
        let config = evaluate(&disk);
        assert_eq!(config.directories(defaults()), defaults());
        assert!(!config.has_file());
        let status = config.status(&disk.stat());
        assert_eq!(status.state, ConfigurationState::Absent);
        assert_eq!(status.file.as_deref(), Some(Path::new(FILE)));
        assert!(status.error.is_none() && !status.changed_since_evaluation);
    }

    #[test]
    fn an_edit_after_the_evaluation_is_reported_as_a_change() {
        let disk = Disk::with(Some(r#"#{ state_dir: "/f/state" }"#));
        let config = evaluate(&disk);
        disk.write(r#"#{ state_dir: "/other/state" }"#);
        let status = config.status(&disk.stat());
        assert!(status.changed_since_evaluation);
        // The app still holds what it evaluated.
        assert_eq!(
            config.directories(defaults()).state_dir,
            PathBuf::from("/f/state")
        );
    }

    #[test]
    fn a_file_that_appears_after_the_evaluation_is_a_change() {
        let disk = Disk::with(None);
        let config = evaluate(&disk);
        disk.write("#{}");
        assert!(config.status(&disk.stat()).changed_since_evaluation);
    }

    #[test]
    fn a_file_that_goes_away_after_the_evaluation_is_a_change() {
        let disk = Disk::with(Some("#{}"));
        let config = evaluate(&disk);
        assert!(config.status(&|_| None).changed_since_evaluation);
    }

    #[test]
    fn the_fingerprint_comes_before_the_read() {
        // A disk that rewrites the file as the app reads it: the app saw the old file's
        // fingerprint, so the new file counts as a change.
        let disk = Disk::with(Some(r#"#{ state_dir: "/f/state" }"#));
        let read = |file: &Path| {
            let text = (disk.read())(file);
            disk.write(r#"#{ state_dir: "/f/state2" }"#);
            text
        };
        let config = AppConfig::evaluate(&location(), &read, &disk.stat(), at(1_000));
        assert!(config.status(&disk.stat()).changed_since_evaluation);
    }

    #[test]
    fn the_configured_tools_replace_the_search_for_the_tools_they_name() {
        let disk = Disk::with(Some(r#"#{ tools: #{ tart: "/f/bin/tart" } }"#));
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tart"), b"").unwrap();
        std::fs::write(dir.path().join("swtpm"), b"").unwrap();
        let search = evaluate(&disk).tool_search(ToolSearch::new(vec![dir.path().into()]));
        // The named tool is found at its location or not at all. The unnamed one is searched.
        assert_eq!(search.find("tart"), None);
        assert_eq!(search.find("swtpm"), Some(dir.path().join("swtpm")));
    }

    #[test]
    fn a_file_that_fails_leaves_the_search_as_it_was() {
        let disk = Disk::with(Some("let x = ;"));
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("tart"), b"").unwrap();
        let search = evaluate(&disk).tool_search(ToolSearch::new(vec![dir.path().into()]));
        assert_eq!(search.find("tart"), Some(dir.path().join("tart")));
    }
}

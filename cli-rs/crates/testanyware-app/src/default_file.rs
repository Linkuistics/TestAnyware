//! The app writes `config.rhai` when it starts and finds no entry (ADR-0021, spec
//! "Configuration").
//!
//! Two properties matter. The app never replaces an entry at the path, and a person never sees a
//! partial file there. One mechanism gives both: write the text to a temporary file in the same
//! directory, then hard-link it to the final name. `link(2)` fails when the name exists, a
//! dangling symlink included, and the name appears with its whole text or not at all. A file
//! system with no hard links fails the write, and the app then runs as with no file.
//!
//! `AppConfig::evaluate` stays a pure reader. The caller writes first and evaluates after, so the
//! app evaluates the file that it wrote.

use std::io::{self, Write};
use std::path::{Path, PathBuf};

use testanyware_config::{default_text, ConfigLocation, CONFIG_ENV};

/// What the write found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Written {
    /// No entry was there, and the file now holds the text.
    Created,
    /// An entry was there, whether a file, an empty file, a directory or a symlink. It is as it was.
    EntryThere,
}

/// The file and the text to write, when this run may write. Only a normal run writes, and only
/// at the default place. A file that `TESTANYWARE_CONFIG` names is the person's choice, and a
/// run in test mode has no `home`.
pub fn plan(
    get: &dyn Fn(&str) -> Option<String>,
    location: &ConfigLocation,
    home: Option<&Path>,
) -> Option<(PathBuf, String)> {
    let named = get(CONFIG_ENV).is_some_and(|value| !value.is_empty());
    match (location, home) {
        (ConfigLocation::File(file), Some(home)) if !named => {
            Some((file.clone(), default_text(home)))
        }
        _ => None,
    }
}

/// Write `text` to `file` unless an entry is there. A missing directory is created.
pub fn write_if_absent(file: &Path, text: &str) -> io::Result<Written> {
    let Some(directory) = file.parent() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the file has no directory",
        ));
    };
    // A file at the path is not worth a directory or a temporary file.
    if file.symlink_metadata().is_ok() {
        return Ok(Written::EntryThere);
    }
    std::fs::create_dir_all(directory)?;
    let temporary = temporary_name(file);
    let result = write_whole(&temporary, text).and_then(|()| std::fs::hard_link(&temporary, file));
    // The temporary file goes whether the link worked or not.
    let _ = std::fs::remove_file(&temporary);
    match result {
        Ok(()) => Ok(Written::Created),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(Written::EntryThere),
        Err(error) => Err(error),
    }
}

/// A name in the directory of `file`, so the link stays on one file system. The process id keeps
/// two apps from sharing one name.
fn temporary_name(file: &Path) -> PathBuf {
    let name = file
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    file.with_file_name(format!(".{name}.{}.tmp", std::process::id()))
}

/// `create_new` refuses a name that exists, a symlink included, so the write cannot follow one.
fn write_whole(path: &Path, text: &str) -> io::Result<()> {
    let mut temporary = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)?;
    temporary.write_all(text.as_bytes())?;
    temporary.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{stat_file, AppConfig};
    use testanyware_app_protocol::ConfigurationState;
    use testanyware_vm::paths::VmPaths;

    const TEXT: &str = "// a file\n#{}\n";

    fn names(directory: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    fn target() -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let file = tmp.path().join(".config/testanyware/config.rhai");
        (tmp, file)
    }

    #[test]
    fn it_writes_when_no_entry_is_there_and_creates_the_missing_directory() {
        let (_tmp, file) = target();
        assert_eq!(write_if_absent(&file, TEXT).unwrap(), Written::Created);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), TEXT);
    }

    #[test]
    fn it_leaves_no_temporary_file() {
        let (_tmp, file) = target();
        write_if_absent(&file, TEXT).unwrap();
        assert_eq!(names(file.parent().unwrap()), ["config.rhai"]);
    }

    #[test]
    fn it_leaves_no_temporary_file_when_an_entry_is_there() {
        let (_tmp, file) = target();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "mine").unwrap();
        write_if_absent(&file, TEXT).unwrap();
        assert_eq!(names(file.parent().unwrap()), ["config.rhai"]);
    }

    #[test]
    fn it_never_replaces_a_file() {
        let (_tmp, file) = target();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "#{ state_dir: \"/mine\" }").unwrap();
        assert_eq!(write_if_absent(&file, TEXT).unwrap(), Written::EntryThere);
        assert_eq!(
            std::fs::read_to_string(&file).unwrap(),
            "#{ state_dir: \"/mine\" }"
        );
    }

    #[test]
    fn it_never_replaces_an_empty_file() {
        let (_tmp, file) = target();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(&file, "").unwrap();
        assert_eq!(write_if_absent(&file, TEXT).unwrap(), Written::EntryThere);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "");
    }

    #[test]
    fn it_never_replaces_a_directory() {
        let (_tmp, file) = target();
        std::fs::create_dir_all(&file).unwrap();
        assert_eq!(write_if_absent(&file, TEXT).unwrap(), Written::EntryThere);
        assert!(file.is_dir());
        assert!(names(&file).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn it_never_replaces_a_dangling_symlink() {
        let (tmp, file) = target();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let gone = tmp.path().join("gone");
        std::os::unix::fs::symlink(&gone, &file).unwrap();
        assert_eq!(write_if_absent(&file, TEXT).unwrap(), Written::EntryThere);
        assert_eq!(std::fs::read_link(&file).unwrap(), gone);
        assert!(!gone.exists(), "the write followed the symlink");
    }

    /// The pre-check is an optimisation. The link is what refuses, so call the second half of
    /// the mechanism on its own: another process wrote the file after the pre-check.
    #[cfg(unix)]
    #[test]
    fn the_link_refuses_an_entry_that_appears_after_the_check() {
        let (tmp, file) = target();
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        let temporary = temporary_name(&file);
        write_whole(&temporary, TEXT).unwrap();
        std::fs::write(&file, "theirs").unwrap();
        let error = std::fs::hard_link(&temporary, &file).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "theirs");
        drop(tmp);
    }

    #[test]
    fn a_failed_write_returns_the_failure_and_leaves_the_path_as_it_was() {
        let tmp = tempfile::tempdir().unwrap();
        // The directory of the file is a file, so no directory can be created there.
        let blocker = tmp.path().join(".config");
        std::fs::write(&blocker, "not a directory").unwrap();
        let file = blocker.join("testanyware/config.rhai");
        assert!(write_if_absent(&file, TEXT).is_err());
        assert_eq!(
            std::fs::read_to_string(&blocker).unwrap(),
            "not a directory"
        );
        assert_eq!(names(tmp.path()), [".config"]);
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_write_into_an_unwritable_directory_leaves_no_temporary_file() {
        use std::os::unix::fs::PermissionsExt;
        let (_tmp, file) = target();
        let directory = file.parent().unwrap();
        std::fs::create_dir_all(directory).unwrap();
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o500)).unwrap();
        // A process that runs as root writes there anyway, and the check would prove nothing.
        let writable = std::fs::write(directory.join("probe"), "").is_ok();
        let result = write_if_absent(&file, TEXT);
        std::fs::set_permissions(directory, std::fs::Permissions::from_mode(0o700)).unwrap();
        if !writable {
            assert!(result.is_err());
            assert!(names(directory).is_empty());
        }
    }

    #[test]
    fn the_app_evaluates_the_file_that_it_wrote_and_sees_no_change() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let file = home.join(".config/testanyware/config.rhai");
        write_if_absent(&file, &default_text(home)).unwrap();
        let config = AppConfig::evaluate_now(&ConfigLocation::File(file.clone()));
        let status = config.status(&stat_file);
        assert_eq!(status.state, ConfigurationState::Loaded);
        assert!(!status.changed_since_evaluation);
        assert!(status.error.is_none());
    }

    #[test]
    fn the_written_file_changes_no_directory_and_no_tool() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let file = home.join(".config/testanyware/config.rhai");
        write_if_absent(&file, &default_text(home)).unwrap();
        let config = AppConfig::evaluate_now(&ConfigLocation::File(file));
        let builtin = VmPaths::builtin_for_home(home);
        let defaults = testanyware_app_protocol::Directories {
            state_dir: builtin.state_dir().to_path_buf(),
            data_dir: builtin.data_dir().to_path_buf(),
        };
        assert_eq!(config.directories(defaults.clone()), defaults);
    }

    #[test]
    fn the_written_file_leaves_the_app_with_both_slots() {
        let tmp = tempfile::tempdir().unwrap();
        let home = tmp.path();
        let file = home.join(".config/testanyware/config.rhai");
        write_if_absent(&file, &default_text(home)).unwrap();
        let config = AppConfig::evaluate_now(&ConfigLocation::File(file));
        assert_eq!(config.macos_slots(), 2);
    }

    #[test]
    fn the_examples_of_the_directories_are_the_built_in_directories_of_the_app() {
        let home = Path::new("/Users/someone");
        let builtin = VmPaths::builtin_for_home(home);
        let text = default_text(home);
        assert!(text.contains(&format!("\"{}\"", builtin.state_dir().display())));
        assert!(text.contains(&format!("\"{}\"", builtin.data_dir().display())));
    }

    fn env<'a>(pairs: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |key| {
            pairs
                .iter()
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value.to_string())
        }
    }

    fn default_location(home: &Path) -> ConfigLocation {
        ConfigLocation::File(home.join(".config/testanyware/config.rhai"))
    }

    #[test]
    fn a_normal_run_at_the_default_place_plans_the_write() {
        let home = Path::new("/h");
        let (file, text) = plan(&env(&[]), &default_location(home), Some(home)).unwrap();
        assert_eq!(file, home.join(".config/testanyware/config.rhai"));
        assert_eq!(text, default_text(home));
    }

    #[test]
    fn a_run_with_testanyware_config_set_writes_nothing_at_the_default_place() {
        let home = Path::new("/h");
        let named = ConfigLocation::File("/elsewhere/config.rhai".into());
        for location in [&named, &default_location(home)] {
            let get = env(&[(CONFIG_ENV, "/elsewhere/config.rhai")]);
            assert_eq!(plan(&get, location, Some(home)), None);
        }
        // `none` disables the file, and the variable still counts as set.
        let get = env(&[(CONFIG_ENV, "none")]);
        assert_eq!(plan(&get, &ConfigLocation::Disabled, Some(home)), None);
    }

    #[test]
    fn an_empty_testanyware_config_counts_as_unset() {
        let home = Path::new("/h");
        let get = env(&[(CONFIG_ENV, "")]);
        assert!(plan(&get, &default_location(home), Some(home)).is_some());
    }

    #[test]
    fn a_run_with_no_file_location_or_no_home_writes_nothing() {
        let home = Path::new("/h");
        assert_eq!(plan(&env(&[]), &ConfigLocation::Disabled, Some(home)), None);
        assert_eq!(plan(&env(&[]), &default_location(home), None), None);
    }
}

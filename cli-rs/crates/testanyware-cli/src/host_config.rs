//! The Host CLI's reading of `config.rhai` (ADR-0021, spec "Configuration").
//!
//! [`resolve`] is the pure function. It takes the activation switch, the environment and a
//! reader, and it gives the status of the file with the directories and tool locations that
//! follow from it. The order of the sources is: flag, environment variable, file, default. The
//! Host CLI has no flag for a directory or a tool, so the order that applies is: environment
//! variable, file, default.
//!
//! A file that fails does not make [`resolve`] fail. It gives [`ConfigStatus::Failed`] and the
//! values that resolve without the file, because `doctor` runs on those values (spec: "doctor
//! reports the failures that it diagnoses"). Every other command calls [`refuse_if_invalid`].

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use testanyware_app_protocol::{AppError, ConfigurationState, ConfigurationStatus, Directories};
use testanyware_config::{load, locate, Activation, Config, ConfigError, ConfigLocation, Loaded, CONFIG_ENV};
use testanyware_vm::paths::{FileDirs, VmPaths};

use crate::output::{exit_code_for, print_error, OutputMode};

/// What the Host CLI made of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigStatus {
    /// The activation switch is off for this host, so the file is not read.
    NotApplicable,
    /// `TESTANYWARE_CONFIG=none`, or no home directory to find the default file in.
    Disabled,
    /// The file does not exist. It is optional.
    Absent {
        file: PathBuf,
    },
    Loaded {
        file: PathBuf,
    },
    Failed(ConfigError),
}

/// The values the Host CLI runs on.
#[derive(Debug, Clone)]
pub struct HostConfig {
    pub status: ConfigStatus,
    /// The directories of the VM lifecycle. Without a usable file they come from the
    /// environment and the host default.
    pub paths: VmPaths,
    /// The directories that the file names, for the code that resolves a directory itself.
    pub file_dirs: FileDirs,
    /// The tool locations that the file names. Empty without a usable file.
    pub tools: Vec<(String, PathBuf)>,
    /// What the TestAnyware app resolves for this user, for the directory check of golden
    /// creation. `None` where the switch is off, or where there is no home directory.
    pub app: Option<AppView>,
}

/// The directories that the app resolves, and the file they come from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppView {
    pub directories: Directories,
    /// The file that the app reads, whether or not it exists.
    pub file: PathBuf,
    /// True when that file exists and evaluates, so that it named the directories.
    pub file_loaded: bool,
}

impl HostConfig {
    /// The `CONFIG_INVALID` error object, when the file fails.
    pub fn invalid(&self) -> Option<AppError> {
        match &self.status {
            ConfigStatus::Failed(error) => Some(AppError::config_invalid(
                &error.file,
                error.line,
                &error.message,
            )),
            _ => None,
        }
    }
}

/// Resolve the settings. `read` returns the text of a file, so a test reads from memory.
pub fn resolve(
    activation: Activation,
    env: &HashMap<String, String>,
    read: &dyn Fn(&Path) -> io::Result<String>,
) -> HostConfig {
    let (status, config) = if activation.is_active() {
        let get = |key: &str| env.get(key).cloned();
        match load(&locate(&get), read) {
            Ok(Loaded::Disabled) => (ConfigStatus::Disabled, None),
            Ok(Loaded::Absent(file)) => (ConfigStatus::Absent { file }, None),
            Ok(Loaded::Present { file, config }) => (ConfigStatus::Loaded { file }, Some(config)),
            Err(error) => (ConfigStatus::Failed(error), None),
        }
    } else {
        (ConfigStatus::NotApplicable, None)
    };
    let file_dirs = config.as_ref().map(dirs_of).unwrap_or_default();
    let tools = config
        .as_ref()
        .map(|c| {
            c.tools
                .named()
                .into_iter()
                .map(|(n, p)| (n.to_string(), p.to_path_buf()))
                .collect()
        })
        .unwrap_or_default();
    HostConfig {
        status,
        paths: VmPaths::from_env_and_file(env, &file_dirs),
        file_dirs,
        tools,
        app: activation.is_active().then(|| app_view(env, read)).flatten(),
    }
}

fn dirs_of(config: &Config) -> FileDirs {
    FileDirs {
        state_dir: config.state_dir.clone(),
        data_dir: config.data_dir.clone(),
    }
}

/// The directories that the app resolves for the user in `env`: those of `config.rhai`, else the
/// built-in defaults of the app. The app has no shell environment, so no flag and no variable
/// reaches this, `TESTANYWARE_CONFIG` included: it names the file of a test, and the app of a
/// person reads the default one. A file that fails leaves the app on its built-in directories.
fn app_view(
    env: &HashMap<String, String>,
    read: &dyn Fn(&Path) -> io::Result<String>,
) -> Option<AppView> {
    let home = env.get("HOME").filter(|home| !home.is_empty())?;
    let ConfigLocation::File(file) = locate(&|key| {
        (key != CONFIG_ENV).then(|| env.get(key).cloned()).flatten()
    }) else {
        return None;
    };
    let loaded = match load(&ConfigLocation::File(file.clone()), read) {
        Ok(Loaded::Present { config, .. }) => Some(config),
        _ => None,
    };
    let builtin = VmPaths::builtin_for_home(Path::new(home));
    let named = loaded.as_ref().map(dirs_of).unwrap_or_default();
    Some(AppView {
        directories: Directories {
            state_dir: named
                .state_dir
                .unwrap_or_else(|| builtin.state_dir().to_path_buf()),
            data_dir: named
                .data_dir
                .unwrap_or_else(|| builtin.data_dir().to_path_buf()),
        },
        file,
        file_loaded: loaded.is_some(),
    })
}

/// Golden creation sends no request, so the app cannot compare its directories with the Host
/// CLI's. On a host that runs the app, golden creation compares them itself before it starts. It
/// needs no running app, because the app's directories follow from `config.rhai` and its
/// built-in defaults alone (spec: "Golden creation keeps its direct path"). A host with no app
/// passes `runs_the_app` as false and the check does nothing.
pub fn check_golden_directories(host: &HostConfig, runs_the_app: bool) -> Result<(), AppError> {
    let Some(app) = host.app.as_ref().filter(|_| runs_the_app) else {
        return Ok(());
    };
    let client = Directories {
        state_dir: host.paths.state_dir().to_path_buf(),
        data_dir: host.paths.data_dir().to_path_buf(),
    };
    if client == app.directories {
        return Ok(());
    }
    let configuration = ConfigurationStatus {
        state: if app.file_loaded {
            ConfigurationState::Loaded
        } else {
            ConfigurationState::Absent
        },
        file: Some(app.file.clone()),
        ..ConfigurationStatus::default()
    };
    Err(AppError::incompatible_directories(
        &app.directories,
        Some(&client),
        &configuration,
    ))
}

static HOST: OnceLock<HostConfig> = OnceLock::new();

/// Resolve the settings of this process once, from its environment and the file system, and
/// install them: the tool locations in the VM crate, and the directories here. The binary calls
/// this first. The first call wins.
pub fn init_from_process() -> &'static HostConfig {
    HOST.get_or_init(|| {
        let env: HashMap<String, String> = std::env::vars_os()
            .filter_map(|(k, v)| Some((k.into_string().ok()?, v.into_string().ok()?)))
            .collect();
        let host = resolve(Activation::for_this_host(), &env, &|file| {
            std::fs::read_to_string(file)
        });
        testanyware_vm::tools::install_configured(host.tools.clone());
        host
    })
}

/// The settings that [`init_from_process`] installed, or `None` where nothing installed them
/// (a test of a library function).
pub fn current() -> Option<&'static HostConfig> {
    HOST.get()
}

/// The directories of the VM lifecycle for this process. Without installed settings they come
/// from the environment alone, as they did before the file existed.
pub fn vm_paths() -> VmPaths {
    match current() {
        Some(host) => host.paths.clone(),
        None => VmPaths::from_process_env(),
    }
}

/// The state directory that the file names, for the code that resolves it from an environment
/// provider of its own.
pub fn file_state_dir() -> Option<PathBuf> {
    current().and_then(|host| host.file_dirs.state_dir.clone())
}

/// Whether the arguments ask for `--json`. The refusal comes before the dispatch to a command, so
/// it reads the arguments itself. An argument after `--` is a value and not a flag.
pub fn json_requested(args: impl IntoIterator<Item = String>) -> bool {
    args.into_iter()
        .take_while(|arg| arg != "--")
        .any(|arg| arg == "--json")
}

/// Every command except `doctor` refuses to run with a file that fails: `CONFIG_INVALID`, exit 2.
pub fn refuse_if_invalid(host: &HostConfig, mode: OutputMode) {
    if let Some(error) = host.invalid() {
        exit_with(&error, mode);
    }
}

/// Golden creation on a macOS host fails with `APP_INCOMPATIBLE` before it starts when its
/// directories differ from the app's.
pub fn refuse_golden_if_directories_differ(mode: OutputMode) {
    let Some(host) = current() else { return };
    if let Err(error) = check_golden_directories(host, cfg!(target_os = "macos")) {
        exit_with(&error, mode);
    }
}

fn exit_with(error: &AppError, mode: OutputMode) -> ! {
    print_error(
        mode,
        &error.code,
        &error.message,
        error.remediation.as_deref(),
        error.details.clone(),
        exit_code_for(&error.code),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// A reader that holds one file, called `file`, with `text` in it.
    fn file_with<'a>(file: &'a str, text: &'a str) -> impl Fn(&Path) -> io::Result<String> + 'a {
        move |path| {
            if path == Path::new(file) {
                Ok(text.to_string())
            } else {
                Err(io::ErrorKind::NotFound.into())
            }
        }
    }

    fn no_file(_: &Path) -> io::Result<String> {
        Err(io::ErrorKind::NotFound.into())
    }

    const FILE: &str = "/h/.config/testanyware/config.rhai";
    const SETTINGS: &str = r#"#{ state_dir: "/f/state", data_dir: "/f/data",
        tools: #{ tart: "/f/tart", "qemu-system-aarch64": "/f/qemu", swtpm: "/f/swtpm" } }"#;

    // ---- the activation switch ----

    #[test]
    fn with_the_switch_off_the_file_is_not_read() {
        let read = |_: &Path| -> io::Result<String> { panic!("an inactive host reads no file") };
        let host = resolve(Activation::Inactive, &env(&[("HOME", "/h")]), &read);
        assert_eq!(host.status, ConfigStatus::NotApplicable);
        assert_eq!(
            host.paths.state_dir(),
            Path::new("/h/.local/state/testanyware")
        );
        assert!(host.tools.is_empty());
        assert!(host.invalid().is_none());
    }

    #[test]
    fn with_the_switch_off_a_file_that_would_fail_changes_nothing() {
        // The same file that fails below with the switch on.
        let host = resolve(
            Activation::Inactive,
            &env(&[("HOME", "/h")]),
            &file_with(FILE, "#{ nope: 1 }"),
        );
        assert_eq!(host.status, ConfigStatus::NotApplicable);
        assert!(host.invalid().is_none());
    }

    #[test]
    fn the_switch_is_on_for_every_host() {
        assert!(Activation::for_this_host().is_active());
    }

    // ---- the directory check of golden creation ----

    fn check(
        env: &[(&str, &str)],
        file: Option<&str>,
        runs_the_app: bool,
    ) -> Result<(), AppError> {
        let read = |path: &Path| match file {
            Some(text) => file_with(FILE, text)(path),
            None => no_file(path),
        };
        let host = resolve(Activation::Active, &self::env(env), &read);
        check_golden_directories(&host, runs_the_app)
    }

    #[test]
    fn golden_passes_when_nothing_moves_a_directory() {
        assert!(check(&[("HOME", "/h")], None, true).is_ok());
    }

    #[test]
    fn golden_passes_when_it_reads_the_file_that_the_app_reads() {
        assert!(check(&[("HOME", "/h")], Some(SETTINGS), true).is_ok());
    }

    #[test]
    fn golden_passes_when_a_variable_names_the_directory_that_the_file_names() {
        let env = [("HOME", "/h"), ("XDG_STATE_HOME", "/f/s")];
        // The state directory of the file is `/f/state`, so `/f/s/testanyware` differs.
        assert!(check(&env, Some(SETTINGS), true).is_err());
        let file = r#"#{ state_dir: "/f/s/testanyware" }"#;
        assert!(check(&env, Some(file), true).is_ok());
    }

    #[test]
    fn golden_fails_on_a_data_home_that_the_file_does_not_name() {
        // The scenario of the spec: XDG_DATA_HOME is set, and the file names no data_dir.
        let env = [("HOME", "/h"), ("XDG_DATA_HOME", "/elsewhere")];
        let error = check(&env, Some(r#"#{ state_dir: "/f/state" }"#), true).unwrap_err();
        assert_eq!(error.code, "APP_INCOMPATIBLE");
        assert_eq!(error.details["cause"], "directories");
        assert_eq!(
            error.details["client"]["data_dir"],
            "/elsewhere/testanyware"
        );
        assert_eq!(error.details["client"]["state_dir"], "/f/state");
        assert_eq!(
            error.details["app"]["data_dir"],
            "/h/.local/share/testanyware"
        );
        assert_eq!(error.details["app"]["state_dir"], "/f/state");
        let remedy = error.remediation.unwrap();
        assert!(remedy.contains("XDG_DATA_HOME"), "{remedy}");
        assert!(!remedy.contains("XDG_STATE_HOME"), "{remedy}");
        assert!(remedy.contains("config.rhai"), "{remedy}");
        // The message names both pairs.
        for needle in ["/elsewhere/testanyware", "/f/state", "/h/.local/share/testanyware"] {
            assert!(error.message.contains(needle), "{needle}: {}", error.message);
        }
    }

    #[test]
    fn golden_fails_when_a_variable_beats_the_file() {
        let env = [("HOME", "/h"), ("XDG_STATE_HOME", "/x")];
        let error = check(&env, Some(SETTINGS), true).unwrap_err();
        assert_eq!(error.details["client"]["state_dir"], "/x/testanyware");
        assert_eq!(error.details["app"]["state_dir"], "/f/state");
    }

    #[test]
    fn the_app_side_of_golden_uses_no_variable() {
        // The app has no shell environment. `TESTANYWARE_CONFIG` names the file of a test, so it
        // moves the file of the Host CLI and leaves the file of the app where it was.
        let other = "/o/other.rhai";
        let read = |path: &Path| {
            if path == Path::new(other) {
                Ok(r#"#{ state_dir: "/o/state" }"#.to_string())
            } else {
                file_with(FILE, SETTINGS)(path)
            }
        };
        let host = resolve(
            Activation::Active,
            &env(&[("HOME", "/h"), ("TESTANYWARE_CONFIG", other), ("XDG_DATA_HOME", "/x")]),
            &read,
        );
        let app = host.app.as_ref().unwrap();
        assert_eq!(app.file, PathBuf::from(FILE));
        assert_eq!(app.directories.state_dir, PathBuf::from("/f/state"));
        assert_eq!(app.directories.data_dir, PathBuf::from("/f/data"));
        // The Host CLI reads the other file, so the two disagree and golden says so.
        assert_eq!(host.paths.state_dir(), Path::new("/o/state"));
        assert!(check_golden_directories(&host, true).is_err());
    }

    #[test]
    fn golden_expects_the_built_in_directories_of_an_app_whose_file_fails() {
        let host = resolve(
            Activation::Active,
            &env(&[("HOME", "/h")]),
            &file_with(FILE, "#{ nope: 1 }"),
        );
        // Every command except `doctor` refuses before golden gets here. The view still says
        // what the app does with such a file.
        let app = host.app.as_ref().unwrap();
        assert_eq!(
            app.directories.state_dir,
            PathBuf::from("/h/.local/state/testanyware")
        );
        assert!(!app.file_loaded);
        assert!(host.invalid().is_some());
    }

    #[test]
    fn golden_does_not_run_the_check_on_a_host_without_the_app() {
        let env = [("HOME", "/h"), ("XDG_DATA_HOME", "/elsewhere")];
        assert!(check(&env, None, true).is_err());
        assert!(check(&env, None, false).is_ok());
    }

    #[test]
    fn golden_has_no_app_to_compare_with_when_the_switch_is_off_or_there_is_no_home() {
        let read = |_: &Path| -> io::Result<String> { panic!("an inactive host reads no file") };
        let host = resolve(
            Activation::Inactive,
            &env(&[("HOME", "/h"), ("XDG_DATA_HOME", "/x")]),
            &read,
        );
        assert!(host.app.is_none());
        assert!(check_golden_directories(&host, true).is_ok());

        let host = resolve(Activation::Active, &env(&[]), &no_file);
        assert!(host.app.is_none());
        assert!(check_golden_directories(&host, true).is_ok());
    }

    // ---- the order of the sources: environment variable, file, default ----

    #[test]
    fn the_file_beats_the_default() {
        let host = resolve(
            Activation::Active,
            &env(&[("HOME", "/h")]),
            &file_with(FILE, SETTINGS),
        );
        assert_eq!(
            host.status,
            ConfigStatus::Loaded {
                file: PathBuf::from(FILE)
            }
        );
        assert_eq!(host.paths.state_dir(), Path::new("/f/state"));
        assert_eq!(host.paths.data_dir(), Path::new("/f/data"));
        assert_eq!(host.paths.vms_dir(), PathBuf::from("/f/state/vms"));
        assert_eq!(host.file_dirs.state_dir, Some(PathBuf::from("/f/state")));
        assert_eq!(
            host.tools,
            vec![
                ("tart".to_string(), PathBuf::from("/f/tart")),
                ("qemu-system-aarch64".to_string(), PathBuf::from("/f/qemu")),
                ("swtpm".to_string(), PathBuf::from("/f/swtpm")),
            ]
        );
    }

    #[test]
    fn the_environment_variable_beats_the_file() {
        let host = resolve(
            Activation::Active,
            &env(&[
                ("HOME", "/h"),
                ("XDG_STATE_HOME", "/s"),
                ("XDG_DATA_HOME", "/d"),
            ]),
            &file_with(FILE, SETTINGS),
        );
        assert_eq!(host.paths.state_dir(), Path::new("/s/testanyware"));
        assert_eq!(host.paths.data_dir(), Path::new("/d/testanyware"));
        // The tools have no variable, so the file still names them.
        assert_eq!(host.tools.len(), 3);
    }

    #[test]
    fn each_setting_follows_its_own_order() {
        let host = resolve(
            Activation::Active,
            &env(&[("HOME", "/h"), ("XDG_DATA_HOME", "/d")]),
            &file_with(FILE, SETTINGS),
        );
        assert_eq!(
            host.paths.state_dir(),
            Path::new("/f/state"),
            "no variable: the file"
        );
        assert_eq!(
            host.paths.data_dir(),
            Path::new("/d/testanyware"),
            "a variable: the variable"
        );
    }

    #[test]
    fn without_a_file_every_setting_is_the_default() {
        let host = resolve(Activation::Active, &env(&[("HOME", "/h")]), &no_file);
        assert_eq!(
            host.status,
            ConfigStatus::Absent {
                file: PathBuf::from(FILE)
            }
        );
        assert_eq!(
            host.paths.state_dir(),
            Path::new("/h/.local/state/testanyware")
        );
        assert_eq!(
            host.paths.data_dir(),
            Path::new("/h/.local/share/testanyware")
        );
        assert!(host.tools.is_empty());
        assert!(host.invalid().is_none());
    }

    #[test]
    fn the_variable_of_the_file_names_another_file_or_none() {
        let host = resolve(
            Activation::Active,
            &env(&[("HOME", "/h"), ("TESTANYWARE_CONFIG", "/t/other.rhai")]),
            &file_with("/t/other.rhai", r#"#{ data_dir: "/o/data" }"#),
        );
        assert_eq!(
            host.status,
            ConfigStatus::Loaded {
                file: PathBuf::from("/t/other.rhai")
            }
        );
        assert_eq!(host.paths.data_dir(), Path::new("/o/data"));

        // `none` makes the Host CLI read no file, even where the default file would fail. The
        // one read that remains is the app's view of the default file, for the golden check.
        let read = |path: &Path| -> io::Result<String> {
            assert_eq!(path, Path::new(FILE), "`none` reads no file for the Host CLI");
            Ok("let a = ;".to_string())
        };
        let host = resolve(
            Activation::Active,
            &env(&[("HOME", "/h"), ("TESTANYWARE_CONFIG", "none")]),
            &read,
        );
        assert_eq!(host.status, ConfigStatus::Disabled);
        assert!(host.invalid().is_none());
    }

    // ---- every error ----

    #[test]
    fn a_file_that_fails_gives_config_invalid_and_the_values_without_the_file() {
        let host = resolve(
            Activation::Active,
            &env(&[("HOME", "/h"), ("XDG_STATE_HOME", "/s")]),
            &file_with(FILE, "let a = 1;\nlet b = ;"),
        );
        let ConfigStatus::Failed(error) = &host.status else {
            panic!("{:?}", host.status)
        };
        assert_eq!(error.line, Some(2));
        let invalid = host.invalid().expect("a failed file is invalid");
        assert_eq!(invalid.code, "CONFIG_INVALID");
        assert_eq!(invalid.details["file"], FILE);
        assert_eq!(invalid.details["line"], 2);
        assert!(
            invalid
                .message
                .starts_with(&format!("{FILE}:2: syntax error")),
            "{}",
            invalid.message
        );
        assert_eq!(exit_code_for(&invalid.code), 2);
        // The values that `doctor` uses: the environment and the defaults, never the file.
        assert_eq!(host.paths.state_dir(), Path::new("/s/testanyware"));
        assert_eq!(
            host.paths.data_dir(),
            Path::new("/h/.local/share/testanyware")
        );
        assert!(host.tools.is_empty());
    }

    #[test]
    fn every_kind_of_error_fails_the_file() {
        for (name, text) in [
            ("a syntax error", "#{"),
            ("a runtime error", "missing_fn()"),
            ("an unknown key", "#{ nope: 1 }"),
            ("an unknown tool", r#"#{ tools: #{ git: "/usr/bin/git" } }"#),
            ("a wrong type", "#{ state_dir: 7 }"),
            ("a relative path", r#"#{ data_dir: "rel" }"#),
            ("not a map", "42"),
            ("an operation loop", "loop {}"),
            ("an import", r#"import "x" as y; #{}"#),
        ] {
            let host = resolve(
                Activation::Active,
                &env(&[("HOME", "/h")]),
                &file_with(FILE, text),
            );
            let invalid = host
                .invalid()
                .unwrap_or_else(|| panic!("{name} should fail: {:?}", host.status));
            assert_eq!(invalid.code, "CONFIG_INVALID", "{name}");
            assert!(invalid.details["message"].is_string(), "{name}");
        }
    }

    #[test]
    fn a_file_that_cannot_be_read_fails_the_file() {
        let denied =
            |_: &Path| -> io::Result<String> { Err(io::ErrorKind::PermissionDenied.into()) };
        let host = resolve(Activation::Active, &env(&[("HOME", "/h")]), &denied);
        let invalid = host.invalid().expect("an unreadable file fails");
        assert!(invalid.details["message"]
            .as_str()
            .unwrap()
            .starts_with("cannot read the file"));
        assert!(invalid.details["line"].is_null());
    }

    // ---- the refusal and the mode ----

    #[test]
    fn the_refusal_reads_json_from_the_arguments_up_to_a_double_dash() {
        let args = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert!(json_requested(args(&["vm", "list", "--json"])));
        assert!(!json_requested(args(&["vm", "list"])));
        assert!(!json_requested(args(&["input", "type", "--", "--json"])));
        assert!(json_requested(args(&[
            "input", "type", "--json", "--", "--json"
        ])));
    }

    #[test]
    fn a_valid_host_does_not_refuse() {
        let host = resolve(
            Activation::Active,
            &env(&[("HOME", "/h")]),
            &file_with(FILE, SETTINGS),
        );
        // `refuse_if_invalid` exits the process on an invalid host, so only the valid case runs.
        refuse_if_invalid(&host, OutputMode::Json);
    }
}

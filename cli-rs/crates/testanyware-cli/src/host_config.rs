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

use testanyware_app_protocol::AppError;
use testanyware_config::{load, locate, Activation, ConfigError, Loaded};
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
    let file_dirs = config
        .as_ref()
        .map(|c| FileDirs {
            state_dir: c.state_dir.clone(),
            data_dir: c.data_dir.clone(),
        })
        .unwrap_or_default();
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
    }
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
        print_error(
            mode,
            &error.code,
            &error.message,
            error.remediation.as_deref(),
            error.details.clone(),
            exit_code_for(&error.code),
        );
    }
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
    fn the_switch_is_on_for_linux_and_windows_hosts_and_off_for_macos() {
        assert_eq!(
            Activation::for_this_host().is_active(),
            !cfg!(target_os = "macos")
        );
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

        // `none` reads nothing, even where the default file would fail.
        let read = |_: &Path| -> io::Result<String> { panic!("`none` reads no file") };
        let host = resolve(
            Activation::Active,
            &env(&[("HOME", "/h"), ("TESTANYWARE_CONFIG", "none")]),
            &read,
        );
        assert_eq!(host.status, ConfigStatus::Disabled);
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

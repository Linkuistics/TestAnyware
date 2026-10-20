//! The evaluator and the location of the file, as pure functions over text.

use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

use testanyware_config::{
    evaluate, load, locate, Config, ConfigError, ConfigLocation, Loaded, ToolLocations, CONFIG_ENV,
    MAX_OPERATIONS,
};

/// An absolute path on any host, and the same path as a Rhai string literal.
fn abs(name: &str) -> PathBuf {
    std::env::temp_dir().join(name)
}

fn lit(path: &Path) -> String {
    format!("{:?}", path.to_str().unwrap())
}

fn file() -> PathBuf {
    PathBuf::from("/home/u/.config/testanyware/config.rhai")
}

fn eval(source: &str) -> Result<Config, ConfigError> {
    evaluate(source, &file())
}

fn failure(source: &str) -> ConfigError {
    eval(source).expect_err("the file should fail")
}

// ---- the value -------------------------------------------------------------------------

#[test]
fn an_empty_map_names_no_setting() {
    assert_eq!(eval("#{}").unwrap(), Config::default());
}

#[test]
fn a_map_names_the_directories_and_the_tools() {
    let (state, data) = (abs("state"), abs("data"));
    let (tart, qemu, swtpm) = (abs("tart"), abs("qemu"), abs("swtpm"));
    let source = format!(
        r#"#{{
            state_dir: {},
            data_dir: {},
            tools: #{{ tart: {}, "qemu-system-aarch64": {}, swtpm: {} }},
        }}"#,
        lit(&state),
        lit(&data),
        lit(&tart),
        lit(&qemu),
        lit(&swtpm),
    );
    assert_eq!(
        eval(&source).unwrap(),
        Config {
            tools: ToolLocations {
                tart: Some(tart.clone()),
                qemu_system_aarch64: Some(qemu.clone()),
                swtpm: Some(swtpm.clone()),
            },
            state_dir: Some(state),
            data_dir: Some(data),
        }
    );
}

#[test]
fn the_script_computes_its_map() {
    let base = abs("base");
    let source = format!(
        r#"let base = {};
           let name = "state";
           #{{ state_dir: base + "/" + name }}"#,
        lit(&base)
    );
    let config = eval(&source).unwrap();
    assert_eq!(
        config.state_dir,
        Some(PathBuf::from(format!("{}/state", base.display())))
    );
}

#[test]
fn named_lists_every_tool_that_the_file_names() {
    let tart = abs("tart");
    let config = eval(&format!("#{{ tools: #{{ tart: {} }} }}", lit(&tart))).unwrap();
    assert_eq!(config.tools.named(), vec![("tart", tart.as_path())]);
    assert_eq!(config.tools.get("swtpm"), None);
    assert_eq!(config.tools.get("not-a-tool"), None);
}

// ---- every error -----------------------------------------------------------------------

#[test]
fn an_unknown_key_is_an_error() {
    let error = failure("#{ state_dirr: \"/x\" }");
    assert_eq!(error.file, file());
    assert_eq!(error.line, None);
    assert!(
        error.message.contains("unknown key `state_dirr`"),
        "{}",
        error.message
    );
    assert!(
        error.message.contains("`tools`"),
        "the error lists the keys: {}",
        error.message
    );
}

#[test]
fn an_unknown_tool_is_an_error() {
    let error = failure("#{ tools: #{ git: \"/usr/bin/git\" } }");
    assert!(
        error.message.contains("unknown tool `git`"),
        "{}",
        error.message
    );
    assert!(error.message.contains("`swtpm`"), "{}", error.message);
}

#[test]
fn a_script_that_is_not_a_map_is_an_error() {
    for (source, found) in [
        ("42", "i64"),
        ("\"text\"", "string"),
        ("[1, 2]", "array"),
        ("", "()"),
    ] {
        let error = failure(source);
        assert!(
            error.message.contains("must evaluate to a map"),
            "{source}: {}",
            error.message
        );
        assert!(error.message.contains(found), "{source}: {}", error.message);
    }
}

#[test]
fn a_setting_of_the_wrong_type_is_an_error() {
    let error = failure("#{ state_dir: 42 }");
    assert!(
        error.message.contains("`state_dir` must be a string"),
        "{}",
        error.message
    );

    let error = failure("#{ tools: \"/x\" }");
    assert!(
        error.message.contains("`tools` must be a map"),
        "{}",
        error.message
    );

    let error = failure("#{ tools: #{ tart: true } }");
    assert!(
        error.message.contains("`tools.tart` must be a string"),
        "{}",
        error.message
    );
}

#[test]
fn an_empty_or_relative_path_is_an_error() {
    let error = failure("#{ data_dir: \"\" }");
    assert!(
        error.message.contains("`data_dir` is empty"),
        "{}",
        error.message
    );

    let error = failure("#{ tools: #{ swtpm: \"bin/swtpm\" } }");
    assert!(
        error
            .message
            .contains("`tools.swtpm` must be an absolute path"),
        "{}",
        error.message
    );
    assert!(error.message.contains("bin/swtpm"), "{}", error.message);
}

#[test]
fn a_syntax_error_names_its_line() {
    let error = failure("let a = 1;\nlet b = ;\n#{}");
    assert_eq!(error.line, Some(2));
    assert!(
        error.message.starts_with("syntax error: "),
        "{}",
        error.message
    );
    assert!(
        !error.message.contains("line 2"),
        "the line is a field: {}",
        error.message
    );
}

#[test]
fn a_runtime_error_names_its_line() {
    let error = failure("let a = 1;\n\nlet b = missing_function(a);\n#{}");
    assert_eq!(error.line, Some(3));
    assert!(
        error.message.contains("missing_function"),
        "{}",
        error.message
    );
}

#[test]
fn a_script_that_loops_stops_at_the_operation_limit() {
    let error = failure("let n = 0;\nloop { n += 1; }\n#{}");
    assert!(
        error.message.contains(&MAX_OPERATIONS.to_string()),
        "{}",
        error.message
    );
    assert_eq!(error.line, Some(2));
}

// ---- no files, no processes, no network ------------------------------------------------

#[test]
fn a_script_cannot_import_a_module() {
    let dir = tempfile_dir();
    std::fs::write(dir.join("other.rhai"), "export const X = 1;").unwrap();
    let source = format!(
        "import {:?} as other;\n#{{}}",
        dir.join("other.rhai").to_str().unwrap()
    );
    let error = failure(&source);
    assert_eq!(error.line, Some(1), "{}", error.message);
    assert!(
        error.message.contains("'import' is a reserved keyword"),
        "{}",
        error.message
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_script_cannot_evaluate_a_string() {
    let error = failure("eval(\"1 + 1\");\n#{}");
    assert_eq!(error.line, Some(1), "{}", error.message);
    assert!(
        error.message.contains("'eval' is disabled"),
        "{}",
        error.message
    );
}

#[test]
fn a_script_can_print_without_effect() {
    // `print` writes to stdout, which carries the `--json` envelope. The engine drops it.
    assert_eq!(
        eval("print(\"hello\"); debug(1); #{}").unwrap(),
        Config::default()
    );
}

#[test]
fn the_script_has_no_clock() {
    let error = failure("let t = timestamp();\n#{}");
    assert_eq!(error.line, Some(1), "{}", error.message);
    assert!(error.message.contains("timestamp"), "{}", error.message);
}

fn tempfile_dir() -> PathBuf {
    let dir = abs(&format!("testanyware-config-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

// ---- the display of an error -----------------------------------------------------------

#[test]
fn an_error_displays_as_file_line_message() {
    let with_line = ConfigError {
        file: file(),
        line: Some(4),
        message: "boom".into(),
    };
    assert_eq!(
        with_line.to_string(),
        format!("{}:4: boom", file().display())
    );
    let without = ConfigError {
        file: file(),
        line: None,
        message: "boom".into(),
    };
    assert_eq!(without.to_string(), format!("{}: boom", file().display()));
}

// ---- the location ----------------------------------------------------------------------

fn env(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<String> {
    let map: HashMap<String, String> = pairs
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    move |key| map.get(key).cloned()
}

#[test]
fn the_default_file_is_in_the_home_directory() {
    assert_eq!(
        locate(&env(&[("HOME", "/Users/a")])),
        ConfigLocation::File(PathBuf::from("/Users/a/.config/testanyware/config.rhai"))
    );
}

#[test]
fn userprofile_stands_in_for_an_unset_home() {
    assert_eq!(
        locate(&env(&[("USERPROFILE", "/Users/a")])),
        ConfigLocation::File(PathBuf::from("/Users/a/.config/testanyware/config.rhai"))
    );
    // An empty HOME counts as unset.
    assert_eq!(
        locate(&env(&[("HOME", ""), ("USERPROFILE", "/Users/b")])),
        ConfigLocation::File(PathBuf::from("/Users/b/.config/testanyware/config.rhai"))
    );
}

#[test]
fn no_home_means_no_file() {
    assert_eq!(locate(&env(&[])), ConfigLocation::Disabled);
}

#[test]
fn the_variable_names_another_file_or_none() {
    assert_eq!(
        locate(&env(&[
            (CONFIG_ENV, "/tmp/other.rhai"),
            ("HOME", "/Users/a")
        ])),
        ConfigLocation::File(PathBuf::from("/tmp/other.rhai"))
    );
    assert_eq!(
        locate(&env(&[(CONFIG_ENV, "none"), ("HOME", "/Users/a")])),
        ConfigLocation::Disabled
    );
    // An empty variable counts as unset.
    assert_eq!(
        locate(&env(&[(CONFIG_ENV, ""), ("HOME", "/Users/a")])),
        ConfigLocation::File(PathBuf::from("/Users/a/.config/testanyware/config.rhai"))
    );
}

// ---- reading ---------------------------------------------------------------------------

#[test]
fn load_reads_a_file_that_exists() {
    let location = ConfigLocation::File(file());
    let loaded = load(&location, &|_| Ok("#{}".to_string())).unwrap();
    assert_eq!(
        loaded,
        Loaded::Present {
            file: file(),
            config: Config::default()
        }
    );
    assert_eq!(loaded.config(), Some(&Config::default()));
}

#[test]
fn load_treats_a_missing_file_as_absent_not_as_an_error() {
    let location = ConfigLocation::File(file());
    let loaded = load(&location, &|_| Err(io::ErrorKind::NotFound.into())).unwrap();
    assert_eq!(loaded, Loaded::Absent(file()));
    assert_eq!(loaded.config(), None);
}

#[test]
fn load_reports_a_file_that_it_cannot_read() {
    let location = ConfigLocation::File(file());
    let error = load(&location, &|_| Err(io::ErrorKind::PermissionDenied.into())).unwrap_err();
    assert_eq!(error.file, file());
    assert!(
        error.message.starts_with("cannot read the file"),
        "{}",
        error.message
    );
}

#[test]
fn load_reports_a_file_that_fails() {
    let location = ConfigLocation::File(file());
    let error = load(&location, &|_| Ok("#{ nope: 1 }".to_string())).unwrap_err();
    assert!(
        error.message.contains("unknown key `nope`"),
        "{}",
        error.message
    );
}

#[test]
fn load_reads_nothing_when_disabled() {
    let loaded = load(&ConfigLocation::Disabled, &|_| {
        panic!("a disabled location reads no file")
    });
    assert_eq!(loaded.unwrap(), Loaded::Disabled);
}

//! The text of the file that the TestAnyware app writes when it finds none (ADR-0021, spec
//! "Configuration").
//!
//! The text is comments around an empty map, so it changes no setting. It is a pure function of
//! the home directory, because the examples of `state_dir` and `data_dir` name the built-in
//! directories under it. The write step belongs to the app.
//!
//! One convention marks the examples. Inside the map, a prose comment is `// ` and a word. An
//! example line is `//` and two spaces. Deleting the `//` of each example line gives a map that
//! evaluates, and a test does exactly that.

use std::path::Path;

/// The text of the default `config.rhai`, for the user whose home directory is `home`.
pub fn default_text(home: &Path) -> String {
    let state_dir = rhai_string(&home.join(".local/state/testanyware"));
    let data_dir = rhai_string(&home.join(".local/share/testanyware"));
    format!(
        r#"// TestAnyware wrote this file because it found no config.rhai. It changes no setting.
// The Host CLI and the TestAnyware app both read this file.
// The app reads it only when it starts, so quit the app and open it again after an edit.
// To use a setting, delete the two slashes in front of its example line.
// TestAnyware never replaces this file. If you delete it, the app writes it again at its next start.
#{{
    // tools: the location of each tool that TestAnyware runs.
    // A tool that you name here is found at that location or not at all.
    // A tool that you do not name is searched in /opt/homebrew/bin, /usr/local/bin and /usr/bin.
    //  tools: #{{
    //      tart: "/opt/homebrew/bin/tart",
    //      "qemu-system-aarch64": "/opt/homebrew/bin/qemu-system-aarch64",
    //      swtpm: "/opt/homebrew/bin/swtpm",
    //  }},

    // state_dir: the directory that holds vms/. This is the built-in directory.
    //  state_dir: "{state_dir}",

    // data_dir: the directory that holds golden/, clones/ and cache/. This is the built-in directory.
    //  data_dir: "{data_dir}",
}}
"#
    )
}

/// `path` as the body of a Rhai string literal. A backslash, a quote or a line break in a home
/// directory would otherwise end the string early, or the comment line.
fn rhai_string(path: &Path) -> String {
    let mut escaped = String::new();
    for c in path.to_string_lossy().chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{evaluate, Config, TOOL_NAMES};
    use std::path::PathBuf;

    const HOME: &str = "/Users/someone";

    fn file() -> PathBuf {
        PathBuf::from(HOME).join(crate::DEFAULT_RELATIVE_PATH)
    }

    fn text() -> String {
        default_text(Path::new(HOME))
    }

    /// An example line is `//`, two spaces and the example. Deleting the `//` keeps the indent.
    fn uncomment_examples(text: &str) -> String {
        text.lines()
            .map(|line| match line.trim_start().strip_prefix("//  ") {
                Some(example) => {
                    let indent = &line[..line.len() - line.trim_start().len()];
                    format!("{indent}  {example}")
                }
                None => line.to_string(),
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn the_text_evaluates_and_sets_no_setting() {
        assert_eq!(evaluate(&text(), &file()), Ok(Config::default()));
    }

    #[test]
    fn the_text_with_the_slashes_of_each_example_deleted_still_evaluates() {
        let uncommented = uncomment_examples(&text());
        assert_ne!(uncommented, text(), "the check deleted nothing");
        let config = evaluate(&uncommented, &file()).expect("the examples evaluate");
        // Every example set something, so the deletion reached all of them.
        assert_eq!(config.tools.named().len(), TOOL_NAMES.len());
        assert!(config.state_dir.is_some() && config.data_dir.is_some());
    }

    #[test]
    fn the_check_of_the_examples_can_fail() {
        // A misspelled key, as the evaluator rejects it.
        let mutated = uncomment_examples(&text()).replace("state_dir:", "stat_dir:");
        assert!(evaluate(&mutated, &file()).is_err());
        // A file of comments alone has no map for a value.
        let comments_only: String = text()
            .lines()
            .filter(|line| line.trim_start().starts_with("//"))
            .map(|line| format!("{line}\n"))
            .collect();
        assert!(evaluate(&comments_only, &file()).is_err());
    }

    #[test]
    fn each_key_of_the_evaluator_appears_in_the_text() {
        let text = text();
        let keys = ["tools", "state_dir", "data_dir"]
            .into_iter()
            .chain(TOOL_NAMES);
        for key in keys {
            assert!(text.contains(key), "the text never names `{key}`");
        }
    }

    #[test]
    fn the_evaluator_accepts_the_keys_that_the_test_expects() {
        // The list in the previous test must not drift from the evaluator. A key that the
        // evaluator does not know fails the file, so each listed key must pass.
        let source = format!(
            "#{{ tools: #{{ {} }}, state_dir: \"/s\", data_dir: \"/d\" }}",
            TOOL_NAMES
                .iter()
                .map(|name| format!("\"{name}\": \"/t/{name}\""))
                .collect::<Vec<_>>()
                .join(", ")
        );
        assert!(evaluate(&source, &file()).is_ok());
    }

    #[test]
    fn the_examples_of_the_directories_name_the_built_in_ones_under_the_home_directory() {
        let config = evaluate(&uncomment_examples(&text()), &file()).unwrap();
        assert_eq!(
            config.state_dir,
            Some(PathBuf::from("/Users/someone/.local/state/testanyware"))
        );
        assert_eq!(
            config.data_dir,
            Some(PathBuf::from("/Users/someone/.local/share/testanyware"))
        );
    }

    #[test]
    fn the_examples_name_each_tool_in_the_homebrew_directory() {
        let config = evaluate(&uncomment_examples(&text()), &file()).unwrap();
        for name in TOOL_NAMES {
            assert_eq!(
                config.tools.get(name),
                Some(PathBuf::from("/opt/homebrew/bin").join(name).as_path())
            );
        }
    }

    #[test]
    fn a_home_directory_with_a_quote_a_backslash_or_a_line_break_still_gives_a_valid_file() {
        let home = PathBuf::from("/Users/a\"b\\c\nd");
        let text = default_text(&home);
        assert_eq!(evaluate(&text, &file()), Ok(Config::default()));
        let config = evaluate(&uncomment_examples(&text), &file()).unwrap();
        assert_eq!(
            config.state_dir,
            Some(home.join(".local/state/testanyware"))
        );
    }

    #[test]
    fn the_prose_says_who_wrote_the_file_and_when_the_app_reads_it() {
        let text = text();
        assert!(text.contains("TestAnyware wrote this file"));
        assert!(text.contains("only when it starts"));
    }
}

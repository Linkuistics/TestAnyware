//! The tool locations of `config.rhai` reach every search of this process (ADR-0021). This test
//! installs them, so it has a binary of its own: the install is once per process.

use testanyware_vm::qemu_profile::which;
use testanyware_vm::tools::{install_configured, ToolSearch};

#[test]
fn an_installed_location_reaches_every_process_search_and_the_first_install_wins() {
    let dir = tempfile::tempdir().unwrap();
    let swtpm = dir.path().join("my-swtpm");
    std::fs::write(&swtpm, b"").unwrap();
    let gone = dir.path().join("no-such-tart");

    assert!(install_configured(vec![
        ("swtpm".into(), swtpm.clone()),
        ("tart".into(), gone),
    ]));

    // The search of the lifecycle code and the `which` of golden creation both see the file.
    assert_eq!(
        ToolSearch::from_process_path().find("swtpm"),
        Some(swtpm.clone())
    );
    assert_eq!(which("swtpm"), Some(swtpm));
    // A configured tool that is no file is not found, and the PATH does not stand in for it.
    assert_eq!(ToolSearch::from_process_path().find("tart"), None);
    // A tool the file does not name still comes from the PATH.
    assert!(ToolSearch::from_process_path().find("sh").is_some());

    assert!(
        !install_configured(vec![]),
        "a second install must not replace the first"
    );
    assert!(ToolSearch::from_process_path().find("swtpm").is_some());
}

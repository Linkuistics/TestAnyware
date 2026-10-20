//! The build of an app bundle, read from its `Info.plist`.
//!
//! The running app reads its own bundle with this, to state `bundle_build`, and `doctor` reads
//! the installed bundle with it. Both then mean the same by the build of a bundle on disk.

use std::path::Path;

/// `CFBundleShortVersionString` of an XML property list. `scripts/build-app.sh` writes the
/// `Info.plist` as XML, and the cask copies the bundle unchanged. A binary property list gives
/// `None`, and the caller then takes the cask upgrade as the remedy, which is always safe.
pub fn bundle_build_from_plist(xml: &str) -> Option<String> {
    const KEY: &str = "<key>CFBundleShortVersionString</key>";
    let after_key = &xml[xml.find(KEY)? + KEY.len()..];
    let value = after_key.split_once("<string>")?.1;
    let (value, _) = value.split_once("</string>")?;
    // The key must be followed by its own value, not by a later key's.
    let between = &after_key[..after_key.find("<string>")?];
    (between.trim().is_empty() && !value.trim().is_empty()).then(|| value.trim().to_string())
}

/// The build of the bundle at `bundle` (the `.app` directory), or `None` when its `Info.plist`
/// does not read.
pub fn bundle_build_at(bundle: &Path) -> Option<String> {
    let plist = bundle.join("Contents").join("Info.plist");
    bundle_build_from_plist(&std::fs::read_to_string(plist).ok()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<plist version="1.0">
<dict>
  <key>CFBundleIdentifier</key>
  <string>dev.antony.TestAnyware</string>
  <key>CFBundleShortVersionString</key>
  <string>1.2.3</string>
  <key>CFBundleVersion</key>
  <string>9.9.9</string>
</dict>
</plist>
"#;

    #[test]
    fn the_build_is_the_short_version_string() {
        assert_eq!(bundle_build_from_plist(PLIST).as_deref(), Some("1.2.3"));
    }

    #[test]
    fn a_plist_without_the_key_or_with_a_binary_body_gives_none() {
        assert_eq!(bundle_build_from_plist("<plist><dict></dict></plist>"), None);
        assert_eq!(bundle_build_from_plist("bplist00\u{1}\u{2}"), None);
        assert_eq!(bundle_build_from_plist(""), None);
    }

    #[test]
    fn a_key_followed_by_another_key_gives_none() {
        let xml = "<key>CFBundleShortVersionString</key><key>Other</key><string>1.0.0</string>";
        assert_eq!(bundle_build_from_plist(xml), None);
    }

    #[test]
    fn a_bundle_directory_gives_the_build_of_its_plist() {
        let dir = tempfile::tempdir().unwrap();
        let bundle = dir.path().join("TestAnyware.app");
        assert_eq!(bundle_build_at(&bundle), None, "no bundle yet");
        std::fs::create_dir_all(bundle.join("Contents")).unwrap();
        std::fs::write(bundle.join("Contents").join("Info.plist"), PLIST).unwrap();
        assert_eq!(bundle_build_at(&bundle).as_deref(), Some("1.2.3"));
    }
}

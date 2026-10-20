//! The build of the app bundle on disk (spec: "Version skew").
//!
//! A cask upgrade replaces the bundle under an app that runs on. The running app then holds the
//! old build, and the bundle holds the new one. The app reads the `Info.plist` of its bundle at
//! each request, so a client learns of the replacement as soon as it happens. The read is of one
//! small file.

use std::path::{Path, PathBuf};

use testanyware_app_protocol::bundle_build_at;

/// Where the app takes the build of its bundle from.
#[derive(Debug, Clone)]
pub enum BundleSource {
    /// The bundle that holds this executable, read at each call.
    Executable(PathBuf),
    /// A fixed answer. The test mode uses it to forge a bundle build, or to state none.
    Fixed(Option<String>),
}

impl BundleSource {
    /// The bundle that holds the running executable.
    pub fn this_process() -> Self {
        match std::env::current_exe() {
            Ok(exe) => Self::Executable(exe),
            Err(_) => Self::Fixed(None),
        }
    }

    /// The build of the bundle on disk now, or `None` when the executable is in no bundle or the
    /// `Info.plist` does not read.
    pub fn build(&self) -> Option<String> {
        match self {
            Self::Fixed(build) => build.clone(),
            Self::Executable(exe) => bundle_build_at(&bundle_dir(exe)?),
        }
    }
}

/// The `.app` directory that holds `exe`, if one does.
fn bundle_dir(exe: &Path) -> Option<PathBuf> {
    exe.ancestors()
        .find(|dir| dir.extension().is_some_and(|ext| ext == "app"))
        .map(Path::to_path_buf)
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
    fn the_bundle_is_the_app_directory_above_the_executable() {
        let exe = Path::new("/Applications/TestAnyware.app/Contents/MacOS/testanyware-app");
        assert_eq!(
            bundle_dir(exe),
            Some(PathBuf::from("/Applications/TestAnyware.app"))
        );
        assert_eq!(
            bundle_dir(Path::new("/x/target/debug/testanyware-app")),
            None
        );
    }

    #[test]
    fn an_executable_reads_the_plist_of_its_bundle_afresh_each_time() {
        let dir = tempfile::tempdir().unwrap();
        let contents = dir.path().join("TestAnyware.app").join("Contents");
        std::fs::create_dir_all(contents.join("MacOS")).unwrap();
        let source = BundleSource::Executable(contents.join("MacOS").join("testanyware-app"));
        assert_eq!(source.build(), None, "no plist yet");
        std::fs::write(contents.join("Info.plist"), PLIST).unwrap();
        assert_eq!(source.build().as_deref(), Some("1.2.3"));
        // A cask upgrade replaces the plist under the running app.
        std::fs::write(contents.join("Info.plist"), PLIST.replace("1.2.3", "1.3.0")).unwrap();
        assert_eq!(source.build().as_deref(), Some("1.3.0"));
    }

    #[test]
    fn a_fixed_source_gives_its_answer() {
        assert_eq!(
            BundleSource::Fixed(Some("4.0.0".into())).build().as_deref(),
            Some("4.0.0")
        );
        assert_eq!(BundleSource::Fixed(None).build(), None);
    }
}

//! Which base VM a macOS golden is cloned from — pure, so it is available on
//! every host (the `--dry-run` plan names the base without touching `tart`).
//!
//! `vm create-golden --platform macos --version <v>` names either a Cirrus Labs
//! **vanilla tag** (`tahoe`, `sequoia`, `sonoma`) or a macOS **major number**
//! (`27`). [`base_source`] decides which by shape: all digits means "build the
//! base from Apple's IPSW" (see [`crate::ipsw_base`]), anything else is a Cirrus
//! tag. The Cirrus tags are never numeric, so that path is unchanged.

/// The Cirrus Labs vanilla image ref for a macOS `version` tag. Ports
/// `_VANILLA="ghcr.io/cirruslabs/macos-$_VERSION-vanilla:latest"`.
pub fn vanilla_image(version: &str) -> String {
    format!("ghcr.io/cirruslabs/macos-{version}-vanilla:latest")
}

/// Where a macOS golden's base VM comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BaseSource {
    /// A Cirrus Labs vanilla image, pulled and cloned by `tart clone`. The
    /// string is the full OCI reference.
    Vanilla(String),
    /// Built here from Apple's IPSW for this macOS major version.
    Ipsw { major: u32 },
}

/// Classify a `--version` value: all-digit → IPSW for that major, otherwise a
/// Cirrus vanilla tag. See the module docs.
pub fn base_source(version: &str) -> BaseSource {
    match version.parse::<u32>() {
        Ok(major) if !version.is_empty() && version.bytes().all(|b| b.is_ascii_digit()) => {
            BaseSource::Ipsw { major }
        }
        _ => BaseSource::Vanilla(vanilla_image(version)),
    }
}

/// The tart VM name of the base built for a macOS major. Deliberately not
/// `testanyware-golden-…` (it is not a golden: no agent, no TCC) and not
/// `testanyware-setup-…` (it outlives any one golden build).
pub fn base_vm_name(major: u32) -> String {
    format!("testanyware-base-macos-{major}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_select_the_ipsw_path_and_names_select_cirrus() {
        assert_eq!(base_source("27"), BaseSource::Ipsw { major: 27 });
        assert_eq!(base_source("14"), BaseSource::Ipsw { major: 14 });
        assert_eq!(
            base_source("tahoe"),
            BaseSource::Vanilla("ghcr.io/cirruslabs/macos-tahoe-vanilla:latest".into())
        );
        assert_eq!(
            base_source("sequoia"),
            BaseSource::Vanilla("ghcr.io/cirruslabs/macos-sequoia-vanilla:latest".into())
        );
    }

    #[test]
    fn mixed_or_empty_versions_stay_on_the_cirrus_path() {
        // Only an all-digit value is an IPSW major; `27.0.1`, `+27`, `` are
        // not, and fall through to the (unchanged) vanilla naming.
        for v in ["27.0.1", "+27", "", "27beta"] {
            assert!(matches!(base_source(v), BaseSource::Vanilla(_)), "{v:?}");
        }
    }

    #[test]
    fn base_vm_name_is_not_a_golden_or_setup_name() {
        let n = base_vm_name(27);
        assert_eq!(n, "testanyware-base-macos-27");
        assert!(!n.starts_with("testanyware-golden-"));
        assert!(!n.starts_with("testanyware-setup-"));
    }
}

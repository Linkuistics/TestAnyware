//! Which macOS major a golden is built for — pure, so it is available on
//! every host (the `--dry-run` plan names the base without touching `tart`).
//!
//! `vm create-golden --platform macos --version <v>` names a macOS **major
//! number** (`27`). [`macos_major`] parses it and refuses anything that is not
//! a major of [`MIN_MACOS_MAJOR`] or later: a Cirrus Labs tag (`tahoe`,
//! `sequoia`, `sonoma`), a major below 27 and a malformed value. The base VM is
//! always built from Apple's IPSW (see [`crate::ipsw_base`]).

use crate::error::VmError;

/// The oldest macOS major that golden creation accepts.
pub const MIN_MACOS_MAJOR: u32 = 27;

/// Parse a `--version` value into a macOS major of [`MIN_MACOS_MAJOR`] or later.
/// Only an all-digit value is a major, so `27.0.1`, `+27`, `27beta` and the empty
/// string fail with the same usage error as `tahoe` and `26`.
pub fn macos_major(version: &str) -> Result<u32, VmError> {
    match version.parse::<u32>() {
        Ok(major)
            if major >= MIN_MACOS_MAJOR && version.bytes().all(|b| b.is_ascii_digit()) =>
        {
            Ok(major)
        }
        _ => Err(VmError::UnsupportedMacosVersion { value: version.to_string() }),
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
    fn a_major_of_27_or_later_selects_that_major() {
        assert_eq!(macos_major("27").unwrap(), 27);
        assert_eq!(macos_major("28").unwrap(), 28);
        assert_eq!(macos_major("127").unwrap(), 127);
    }

    #[test]
    fn a_cirrus_tag_or_an_older_major_is_a_usage_error_that_names_macos_27() {
        for v in ["tahoe", "sequoia", "sonoma", "26", "14", "0"] {
            let err = macos_major(v).unwrap_err();
            assert!(
                matches!(&err, VmError::UnsupportedMacosVersion { value } if value == v),
                "{v:?}: {err:?}"
            );
            assert_eq!(err.exit_code(), 2, "{v:?}");
            assert_eq!(err.code(), "USAGE_ERROR", "{v:?}");
            assert!(err.to_string().contains("macOS 27"), "{v:?}: {err}");
        }
    }

    #[test]
    fn a_value_that_is_not_all_digits_is_refused() {
        // `27.0.1`, `+27`, `27beta`, `` and a number too large for a u32.
        for v in ["27.0.1", "+27", "", "27beta", " 27", "99999999999"] {
            assert!(macos_major(v).is_err(), "{v:?}");
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

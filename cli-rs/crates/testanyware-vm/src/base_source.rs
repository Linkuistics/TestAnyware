//! The pure decisions of macOS golden creation (ADR-0027). They need no `tart` and no network, so
//! they build and test on every host.
//!
//! `vm create-golden --platform macos --version <v>` names a macOS **major number** (`27`).
//! [`macos_major`] parses it and refuses anything that is not a major of [`MIN_MACOS_MAJOR`] or
//! later: a Cirrus Labs tag (`tahoe`, `sequoia`, `sonoma`), a major below 27 and a malformed value.
//!
//! [`plan_golden`] takes the text of Apple's IPSW catalogue and the names that `tart list` gives.
//! It returns the release to install, the name of its base VM, and the older bases that the run
//! deletes. The fast checks, the build and the dry run all use this one plan. The I/O around it is
//! in [`crate::ipsw_base`].

use crate::error::VmError;

/// The oldest macOS major that golden creation accepts.
pub const MIN_MACOS_MAJOR: u32 = 27;

/// Apple's macOS IPSW catalogue, the source that `tart create --from-ipsw latest` reads.
pub const IPSW_CATALOGUE_URL: &str =
    "https://mesu.apple.com/assets/macos/com_apple_macOSIPSW/com_apple_macOSIPSW.xml";

/// The hardware model of a virtual Mac. A build of the catalogue serves a virtual Mac only when
/// it is listed under this model.
pub const VIRTUAL_MAC: &str = "VirtualMac2,1";

/// The start of the name of each base VM. It is not `testanyware-golden-`, because a base is not
/// a golden: it has no agent and no TCC grants. It is not `testanyware-setup-`, because a base
/// outlives each golden build.
pub const BASE_PREFIX: &str = "testanyware-base-macos-";

/// Parse a `--version` value into a macOS major of [`MIN_MACOS_MAJOR`] or later.
/// Only an all-digit value is a major, so `27.0.1`, `+27`, `27beta` and the empty
/// string fail with the same usage error as `tahoe` and `26`.
pub fn macos_major(version: &str) -> Result<u32, VmError> {
    match version.parse::<u32>() {
        Ok(major) if major >= MIN_MACOS_MAJOR && version.bytes().all(|b| b.is_ascii_digit()) => {
            Ok(major)
        }
        _ => Err(VmError::UnsupportedMacosVersion {
            value: version.to_string(),
        }),
    }
}

/// One macOS restore image that the catalogue lists for a virtual Mac.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpswRelease {
    /// The macOS version, for example `27.2.1`.
    pub version: String,
    /// The build, for example `26B131`.
    pub build: String,
    /// The `FirmwareURL` of the restore image.
    pub url: String,
}

impl IpswRelease {
    /// The macOS major version (`27` for `27.2.1`).
    pub fn major(&self) -> Option<u32> {
        self.version.split('.').next()?.parse().ok()
    }

    /// The file name of the cached restore image.
    pub fn cache_file_name(&self) -> String {
        format!("macos-{}-{}.ipsw", self.version, self.build)
    }
}

/// Each release that the catalogue lists under [`VIRTUAL_MAC`]. The path to a release is
/// `MobileDeviceSoftwareVersionsByVersion` → each group → `MobileDeviceSoftwareVersions` →
/// `VirtualMac2,1` → each build → `Restore`. An entry with no `Restore` dictionary, or with a
/// missing `ProductVersion`, `BuildVersion` or `FirmwareURL`, is not a release. Each hardware
/// model of a real Mac also holds an `Unknown` key, and this read never looks there.
///
/// Text that is not an XML property list fails, and the error says why.
pub fn virtual_mac_releases(catalogue: &str) -> Result<Vec<IpswRelease>, String> {
    let root = plist::Value::from_reader_xml(catalogue.as_bytes()).map_err(|e| e.to_string())?;
    let mut releases = Vec::new();
    let groups = root
        .as_dictionary()
        .and_then(|d| d.get("MobileDeviceSoftwareVersionsByVersion"))
        .and_then(plist::Value::as_dictionary);
    for (_, group) in groups.into_iter().flatten() {
        let builds = group
            .as_dictionary()
            .and_then(|d| d.get("MobileDeviceSoftwareVersions"))
            .and_then(plist::Value::as_dictionary)
            .and_then(|d| d.get(VIRTUAL_MAC))
            .and_then(plist::Value::as_dictionary);
        for (_, entry) in builds.into_iter().flatten() {
            let Some(restore) = entry
                .as_dictionary()
                .and_then(|d| d.get("Restore"))
                .and_then(plist::Value::as_dictionary)
            else {
                continue;
            };
            let field = |key: &str| restore.get(key).and_then(plist::Value::as_string);
            let (Some(version), Some(build), Some(url)) = (
                field("ProductVersion"),
                field("BuildVersion"),
                field("FirmwareURL"),
            ) else {
                continue;
            };
            let release = IpswRelease {
                version: version.to_string(),
                build: build.to_string(),
                url: url.to_string(),
            };
            if !releases.contains(&release) {
                releases.push(release);
            }
        }
    }
    Ok(releases)
}

/// The numeric parts of a version, so that `27.0.10` orders after `27.0.2`.
fn version_key(version: &str) -> Vec<u32> {
    version.split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

/// A build as (train, letter, number), so that `26B131` orders after `26B99`. A build that does
/// not have this shape orders by its text, after each build that has it.
fn build_key(build: &str) -> (u32, String, u32, String) {
    let digits = |s: &str| s.bytes().take_while(u8::is_ascii_digit).count();
    let train_len = digits(build);
    let rest = &build[train_len..];
    let letters_len = rest.bytes().take_while(u8::is_ascii_alphabetic).count();
    let (letters, tail) = rest.split_at(letters_len);
    let number_len = digits(tail);
    let (number, suffix) = tail.split_at(number_len);
    match (build[..train_len].parse(), number.parse()) {
        (Ok(train), Ok(number)) => (train, letters.to_string(), number, suffix.to_string()),
        _ => (u32::MAX, build.to_string(), u32::MAX, String::new()),
    }
}

/// The newest release of macOS `major` in `releases`, by version and then by build, so that a tie
/// has one answer. `None` when the catalogue lists none.
pub fn select_release(releases: &[IpswRelease], major: u32) -> Option<&IpswRelease> {
    releases
        .iter()
        .filter(|r| r.major() == Some(major))
        .max_by(|a, b| {
            version_key(&a.version)
                .cmp(&version_key(&b.version))
                .then_with(|| build_key(&a.build).cmp(&build_key(&b.build)))
        })
}

/// The tart VM name of the base of a release: `testanyware-base-macos-27.2.1-26B131`. The name
/// carries the release, so `tart list` tells it with no boot and no second record.
pub fn base_vm_name(release: &IpswRelease) -> String {
    format!("{BASE_PREFIX}{}-{}", release.version, release.build)
}

/// The macOS major of a base VM name: the first number after [`BASE_PREFIX`]. `None` for a name
/// that is not a base.
pub fn base_major(name: &str) -> Option<u32> {
    let rest = name.strip_prefix(BASE_PREFIX)?;
    let len = rest.bytes().take_while(u8::is_ascii_digit).count();
    rest[..len].parse().ok()
}

/// The older bases of macOS `major` in `names`: each base of that major that is not `current`.
/// The class holds the name `testanyware-base-macos-27` of 2.9.0, and a `-building` VM of any
/// release of the major. It never holds a base of another major, such as 28 or 270.
pub fn older_bases(names: &[String], major: u32, current: &str) -> Vec<String> {
    names
        .iter()
        .filter(|n| n.as_str() != current && base_major(n) == Some(major))
        .cloned()
        .collect()
}

/// The cached restore images that a refresh deletes: each `macos-*.ipsw` and `macos-*.ipsw.part`
/// file of a release other than `keep`. Another file in the cache is never in the list.
pub fn stale_ipsw_files(file_names: &[String], keep: &IpswRelease) -> Vec<String> {
    let kept = keep.cache_file_name();
    let kept_part = format!("{kept}.part");
    file_names
        .iter()
        .filter(|n| n.starts_with("macos-") && (n.ends_with(".ipsw") || n.ends_with(".ipsw.part")))
        .filter(|n| **n != kept && **n != kept_part)
        .cloned()
        .collect()
}

/// What one macOS golden creation does, decided before it changes anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoldenPlan {
    /// The release that the base holds.
    pub release: IpswRelease,
    /// The name of the base of that release.
    pub base: String,
    /// Whether `tart list` names that base.
    pub base_exists: bool,
    /// The older bases of the major. The run deletes them when the base is ready.
    pub older: Vec<String>,
    /// Whether `tart list` names the golden. The run replaces it when the new golden is ready.
    pub golden_exists: bool,
}

impl GoldenPlan {
    /// The name `<base>-building` of the VM that a base build boots, when the base does not exist.
    pub fn building(&self) -> Option<String> {
        (!self.base_exists).then(|| format!("{}-building", self.base))
    }
}

/// The plan of a macOS golden creation from the catalogue text, the names of `tart list`, the
/// major and the golden name. The release is the newest one that the catalogue lists under
/// [`VIRTUAL_MAC`] for the major. When the catalogue has no such entry, the plan fails with
/// `GOLDEN_CREATE_FAILED`, and the error names the catalogue and the major. The plan never falls
/// back to another build.
pub fn plan_golden(
    catalogue: &str,
    names: &[String],
    major: u32,
    golden: &str,
) -> Result<GoldenPlan, VmError> {
    let releases = virtual_mac_releases(catalogue).map_err(|e| VmError::GoldenCreateFailed {
        detail: format!("Apple's IPSW catalogue {IPSW_CATALOGUE_URL} is not a property list: {e}"),
    })?;
    let release =
        select_release(&releases, major)
            .cloned()
            .ok_or_else(|| VmError::GoldenCreateFailed {
                detail: format!(
                "Apple's IPSW catalogue {IPSW_CATALOGUE_URL} lists no {VIRTUAL_MAC} restore image \
                 of macOS {major}. Use a macOS major that the catalogue lists for a virtual Mac."
            ),
            })?;
    let base = base_vm_name(&release);
    Ok(GoldenPlan {
        base_exists: names.contains(&base),
        older: older_bases(names, major, &base),
        golden_exists: names.iter().any(|n| n == golden),
        release,
        base,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trimmed copy of the catalogue of 2026-11-21. It keeps the real structure, the
    /// `VirtualMac2,1` entry, and one hardware model each for 26B7131 and 26B8132.
    const CATALOGUE: &str = include_str!("../tests/fixtures/ipsw-catalogue-2026-11-21.xml");

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    fn release(version: &str, build: &str) -> IpswRelease {
        IpswRelease {
            version: version.into(),
            build: build.into(),
            url: format!("https://x/{build}"),
        }
    }

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
    fn the_catalogue_read_finds_the_virtual_mac_release_only() {
        let releases = virtual_mac_releases(CATALOGUE).unwrap();
        assert_eq!(
            releases,
            [IpswRelease {
                version: "27.2.1".into(),
                build: "26B131".into(),
                url: "https://updates.cdn-apple.com/2026FallFCS/c202a724-db7e-4d56-917b-110aa0226507/UniversalMac_27.2.1_26B131_Restore.ipsw".into(),
            }]
        );
        // The control: the two builds for new Mac models are in the copy, under other models.
        assert!(CATALOGUE.contains("26B7131") && CATALOGUE.contains("26B8132"));
    }

    #[test]
    fn the_plan_of_2026_11_21_selects_27_2_1_26b131() {
        let plan = plan_golden(CATALOGUE, &[], 27, "testanyware-golden-macos-27").unwrap();
        assert_eq!(plan.release.version, "27.2.1");
        assert_eq!(plan.release.build, "26B131");
        assert_eq!(plan.base, "testanyware-base-macos-27.2.1-26B131");
        assert!(!plan.base_exists);
        assert!(plan.older.is_empty());
        assert!(!plan.golden_exists);
        assert_eq!(
            plan.building().as_deref(),
            Some("testanyware-base-macos-27.2.1-26B131-building")
        );
    }

    #[test]
    fn a_catalogue_with_no_virtual_mac_entry_for_the_major_fails_and_names_the_catalogue() {
        // Each `VirtualMac2,1` key becomes another model, so only builds for real Macs remain.
        let no_virtual_mac = CATALOGUE.replace("<key>VirtualMac2,1</key>", "<key>Mac99,1</key>");
        for (text, major) in [(no_virtual_mac.as_str(), 27), (CATALOGUE, 28)] {
            let err = plan_golden(text, &[], major, "g").unwrap_err();
            assert_eq!(err.code(), "GOLDEN_CREATE_FAILED", "{err:?}");
            let detail = err.details()["detail"].as_str().unwrap().to_string();
            assert!(detail.contains(IPSW_CATALOGUE_URL), "{detail}");
            assert!(detail.contains(&format!("macOS {major}")), "{detail}");
        }
    }

    #[test]
    fn text_that_is_not_a_property_list_fails_and_names_the_catalogue() {
        let err = plan_golden("<html>busy</html>", &[], 27, "g").unwrap_err();
        assert_eq!(err.code(), "GOLDEN_CREATE_FAILED");
        assert!(err.details()["detail"]
            .as_str()
            .unwrap()
            .contains(IPSW_CATALOGUE_URL));
    }

    #[test]
    fn selection_orders_by_version_and_then_by_build() {
        let releases = [
            release("27.0.2", "26A500"),
            release("27.0.10", "26A999"),
            release("26.1", "25B78"),
            release("27.0.10", "26A1000"),
        ];
        // 27.0.10 is newer than 27.0.2 by number, not by text. 26A1000 is newer than 26A999.
        let chosen = select_release(&releases, 27).unwrap();
        assert_eq!(
            (chosen.version.as_str(), chosen.build.as_str()),
            ("27.0.10", "26A1000")
        );
        // The order of the list does not change the answer.
        let mut reversed = releases.clone();
        reversed.reverse();
        assert_eq!(select_release(&reversed, 27), Some(chosen));
        assert_eq!(
            select_release(&releases, 26).map(|r| r.build.as_str()),
            Some("25B78")
        );
        assert_eq!(select_release(&releases, 28), None);
    }

    #[test]
    fn the_base_name_carries_the_release() {
        let n = base_vm_name(&release("27.2.1", "26B131"));
        assert_eq!(n, "testanyware-base-macos-27.2.1-26B131");
        assert!(!n.starts_with("testanyware-golden-"));
        assert!(!n.starts_with("testanyware-setup-"));
        assert_eq!(base_major(&n), Some(27));
    }

    #[test]
    fn older_bases_are_the_other_bases_of_the_major() {
        let current = "testanyware-base-macos-27.2.1-26B131";
        let listed = names(&[
            "testanyware-base-macos-27",
            current,
            "testanyware-base-macos-27.0.1-26A434-building",
            "testanyware-base-macos-27.2.1-26B131-building",
            "testanyware-base-macos-28.0-27A100",
            "testanyware-base-macos-270",
            "testanyware-base-macos-2",
            "testanyware-golden-macos-27",
            "testanyware-base-linux-27",
            "testanyware-base-macos-",
        ]);
        assert_eq!(
            older_bases(&listed, 27, current),
            names(&[
                "testanyware-base-macos-27",
                "testanyware-base-macos-27.0.1-26A434-building",
                "testanyware-base-macos-27.2.1-26B131-building",
            ])
        );
        assert!(older_bases(&names(&[current]), 27, current).is_empty());
    }

    #[test]
    fn the_plan_names_an_existing_base_golden_and_older_base() {
        let listed = names(&["testanyware-base-macos-27", "testanyware-golden-macos-27"]);
        let plan = plan_golden(CATALOGUE, &listed, 27, "testanyware-golden-macos-27").unwrap();
        assert!(!plan.base_exists);
        assert_eq!(plan.older, names(&["testanyware-base-macos-27"]));
        assert!(plan.golden_exists);

        let listed = names(&["testanyware-base-macos-27.2.1-26B131"]);
        let plan = plan_golden(CATALOGUE, &listed, 27, "testanyware-golden-macos-27").unwrap();
        assert!(plan.base_exists);
        assert!(plan.older.is_empty());
        assert_eq!(plan.building(), None);
    }

    #[test]
    fn a_refresh_deletes_the_cached_images_of_other_releases_only() {
        let keep = release("27.2.1", "26B131");
        let files = names(&[
            "macos-27.2.1-26B131.ipsw",
            "macos-27.0.1-26A434.ipsw",
            "macos-27.0.1-26A434.ipsw.part",
            "macos-28.0-27A100.ipsw",
            "macos-27.2.1-26B131.ipsw.part",
            "ubuntu-24.04.img",
            "macos-notes.txt",
        ]);
        assert_eq!(
            stale_ipsw_files(&files, &keep),
            names(&[
                "macos-27.0.1-26A434.ipsw",
                "macos-27.0.1-26A434.ipsw.part",
                "macos-28.0-27A100.ipsw"
            ])
        );
    }

    #[test]
    fn the_cache_file_names_version_and_build() {
        let r = release("27.0.1", "26A434");
        assert_eq!(r.major(), Some(27));
        assert_eq!(r.cache_file_name(), "macos-27.0.1-26A434.ipsw");
    }
}

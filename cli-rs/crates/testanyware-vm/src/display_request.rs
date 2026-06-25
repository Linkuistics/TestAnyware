//! Parse the `--display` flag, splitting the HiDPI `@2x` opt-in (ADR-0016 D3)
//! from the value handed to the backend display config.
//!
//! This is the **mechanism-agnostic** front of the HiDPI opt-in: `@2x` is a
//! scale suffix we parse and translate ourselves — tart never sees it (ADR-0016
//! D3). Today `@2x` maps to tart's host-scale `pt` path (`WxH@2x` → `WxHpt`,
//! which inherits the host monitor's backing scale, k4); when the deferred
//! deterministic mechanism lands (ADR-0016 "Deferred") only this translation
//! changes, not the user surface.
//!
//! Pure and **not** macOS-gated so the CLI boundary can validate `@Nx`
//! uniformly on any host before routing; the macOS-only guest-side 2× switch
//! lives in [`crate::display`].

use thiserror::Error;

/// A parsed `--display` request.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DisplayRequest {
    /// The value to hand the backend display config (`tart set --display` /
    /// QEMU). Never carries `@2x` — under HiDPI it is the translated `WxHpt`.
    /// `None` means the backend default applies (ADR-0013).
    pub backend_display: Option<String>,
    /// The HiDPI **logical** target `(width, height)` in points, set only when
    /// `@2x` was requested (ADR-0016 D2/D3). The integer scale is *not* stored:
    /// k5's connection auto-detects it from the live physical framebuffer, so a
    /// 1× host (where HiDPI did not take) degrades to a no-op.
    pub logical: Option<(u32, u32)>,
}

/// A `--display` parse failure. Both arms are usage errors (the CLI maps them
/// to `USAGE_ERROR`, exit 2) with an actionable remediation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum DisplayParseError {
    /// A `@Nx` suffix for some N≠2. Only an exact integer `@2x` lands on the
    /// vision distribution (ADR-0016 D3); fractional/other scaling is out of
    /// scope.
    #[error("unsupported display scale '@{scale}' in '{value}'; only '@2x' (integer 2× HiDPI) is supported")]
    UnsupportedScale { value: String, scale: String },

    /// `@2x` with a malformed point size before the `@` (non-numeric, zero, or
    /// carrying a unit suffix — `@2x` already implies logical points).
    #[error(
        "malformed HiDPI display '{value}'; expected WxH@2x with positive integer \
         width and height and no unit suffix, e.g. 1920x1080@2x"
    )]
    MalformedHidpi { value: String },
}

impl DisplayParseError {
    /// A one-line, actionable remediation for the error envelope.
    pub fn remediation(&self) -> &'static str {
        match self {
            DisplayParseError::UnsupportedScale { .. } => {
                "Use WxH@2x for HiDPI/Retina, or WxH for the default 1× resolution."
            }
            DisplayParseError::MalformedHidpi { .. } => {
                "Pass the logical size with no unit suffix, e.g. --display 1920x1080@2x."
            }
        }
    }
}

/// Parse a raw `--display` value into a [`DisplayRequest`].
///
/// - `None` → the backend default, no HiDPI.
/// - `WxH` (no `@`) → passed through verbatim to the backend (e.g. `1920x1080`,
///   `1920x1080px`, `800x600`); the legacy px/pt unit handling is unchanged.
/// - `WxH@2x` → the HiDPI opt-in: logical target `(W, H)`, backend value
///   translated to `WxHpt` so tart routes to the host-scale path (ADR-0016 D3).
/// - `WxH@Nx` for N≠2, or a malformed `@2x` size → an error.
pub fn parse_display_request(raw: Option<&str>) -> Result<DisplayRequest, DisplayParseError> {
    let Some(raw) = raw else {
        return Ok(DisplayRequest::default());
    };
    match raw.split_once('@') {
        // No scale suffix: pass the value through to the backend untouched.
        None => Ok(DisplayRequest {
            backend_display: Some(raw.to_string()),
            logical: None,
        }),
        Some((base, scale)) => {
            if scale != "2x" {
                return Err(DisplayParseError::UnsupportedScale {
                    value: raw.to_string(),
                    scale: scale.to_string(),
                });
            }
            let (w, h) = parse_logical_dims(base).ok_or_else(|| DisplayParseError::MalformedHidpi {
                value: raw.to_string(),
            })?;
            Ok(DisplayRequest {
                // Translate to the host-scale `pt` path; tart never sees `@2x`.
                backend_display: Some(format!("{w}x{h}pt")),
                logical: Some((w, h)),
            })
        }
    }
}

/// Parse a strict `WIDTHxHEIGHT` of positive integers — **no** `px`/`pt` suffix
/// (the `@2x` form already fixes the interpretation as logical points, so a
/// suffix is contradictory and rejected as malformed).
fn parse_logical_dims(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    let w: u32 = w.parse().ok()?;
    let h: u32 = h.parse().ok()?;
    (w != 0 && h != 0).then_some((w, h))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_is_the_backend_default_with_no_hidpi() {
        assert_eq!(parse_display_request(None).unwrap(), DisplayRequest::default());
    }

    #[test]
    fn plain_resolution_passes_through_untouched() {
        // The legacy 1× path is byte-identical: the value reaches the backend
        // verbatim and no logical target is set.
        for raw in ["1920x1080", "1920x1080px", "1920x1080pt", "800x600"] {
            let r = parse_display_request(Some(raw)).unwrap();
            assert_eq!(r.backend_display.as_deref(), Some(raw));
            assert_eq!(r.logical, None);
        }
    }

    #[test]
    fn at_2x_sets_logical_and_translates_to_pt() {
        let r = parse_display_request(Some("1920x1080@2x")).unwrap();
        // tart never sees @2x; it gets the host-scale pt path (ADR-0016 D3).
        assert_eq!(r.backend_display.as_deref(), Some("1920x1080pt"));
        assert_eq!(r.logical, Some((1920, 1080)));
    }

    #[test]
    fn at_2x_works_for_non_1080_logical_sizes() {
        let r = parse_display_request(Some("1280x720@2x")).unwrap();
        assert_eq!(r.backend_display.as_deref(), Some("1280x720pt"));
        assert_eq!(r.logical, Some((1280, 720)));
    }

    #[test]
    fn rejects_non_2x_scales() {
        for raw in ["1920x1080@1x", "1920x1080@3x", "1920x1080@2", "1920x1080@x", "1920x1080@2x@foo"] {
            let err = parse_display_request(Some(raw)).expect_err(raw);
            assert!(
                matches!(err, DisplayParseError::UnsupportedScale { .. }),
                "{raw} should be UnsupportedScale, got {err:?}"
            );
        }
    }

    #[test]
    fn rejects_malformed_at_2x_size() {
        // Empty, missing a dimension, zero, or a contradictory unit suffix.
        for raw in ["@2x", "1920@2x", "1920x@2x", "0x1080@2x", "1920x0@2x", "1920x1080px@2x"] {
            let err = parse_display_request(Some(raw)).expect_err(raw);
            assert!(
                matches!(err, DisplayParseError::MalformedHidpi { .. }),
                "{raw} should be MalformedHidpi, got {err:?}"
            );
        }
    }

    #[test]
    fn error_messages_name_the_offending_value() {
        let err = parse_display_request(Some("1920x1080@3x")).unwrap_err();
        assert!(err.to_string().contains("1920x1080@3x"));
        assert!(err.to_string().contains("@2x"));
        assert!(!err.remediation().is_empty());
    }
}

//! macOS guest runtime display-resolution switch (ADR-0014).
//!
//! A macOS Virtualization.framework guest's framebuffer is
//! [[Guest-controlled resolution]]: WindowServer restores the golden's saved
//! mode (1024×768) on login and VF sizes the [[Host-side framebuffer]] to it,
//! so the host-side `tart set --display` is only a hint. To reach the
//! 1920×1080-px [[Framebuffer-pixel contract]] without regenerating the golden,
//! `vm start` (after the agent is ready) host-compiles a small CoreGraphics
//! helper, uploads it over the agent's `/upload` surface, and `/exec`s it to
//! switch the guest's main display to the 1× mode of the target px size.
//!
//! The mechanism was de-risked by the `spike-display-modes-k2` spike and
//! CONFIRMED — see ADR-0014's Verification section. This module is the
//! production build of that spike.
//!
//! macOS-host only, like [`crate::tart`]: the helper is Swift/CoreGraphics and
//! the whole flow only applies to a macOS guest under tart.

use std::time::Duration;

use testanyware_agent_client::{AgentClient, AgentConfig, AgentError};
use testanyware_protocol::ExecRequest;

use crate::paths::VmPaths;

/// The CoreGraphics switch helper, host-compiled with `swiftc` and uploaded to
/// the guest. Embedded so the binary is self-contained (mirrors
/// `golden::SET_WALLPAPER_SWIFT`).
const SET_DISPLAY_MODE_SWIFT: &str =
    include_str!("../../../../provisioner/helpers/set-display-mode.swift");

/// Parse a resolved `--display` value (`WIDTHxHEIGHT[px|pt]`) into the pixel
/// target the CoreGraphics switch helper selects its 1× mode by.
///
/// The unit suffix is **ignored**: VF advertises only 1× (LoDPI) modes here,
/// where `pixelWidth == width` (px == pt — ADR-0014 Verification finding 1), so
/// the numeric `WIDTHxHEIGHT` *is* the px target whether the user wrote `px`,
/// `pt`, or nothing. The same resolved value feeds `tart set --display` and
/// this helper, so the two stay consistent. The pre-existing macOS pt/px
/// `--display` footgun (ADR-0013 — a bare `1920x1080` on a *HiDPI* guest would
/// mean points → a 2× framebuffer) is **not** resolved here: this grove targets
/// the default path, VF offers no HiDPI mode, and a request with no matching 1×
/// mode degrades cleanly (the helper exits non-zero and the caller warns).
///
/// Returns `None` for anything that is not `<digits>x<digits>` with an optional
/// `px`/`pt` suffix, or whose dimensions are zero.
pub fn parse_target(display: &str) -> Option<(u32, u32)> {
    let dims = display
        .strip_suffix("px")
        .or_else(|| display.strip_suffix("pt"))
        .unwrap_or(display);
    let (w, h) = dims.split_once('x')?;
    let w: u32 = w.parse().ok()?;
    let h: u32 = h.parse().ok()?;
    if w == 0 || h == 0 {
        return None;
    }
    Some((w, h))
}

/// Best-effort macOS guest resolution switch over the agent (ADR-0014/0016).
///
/// Host-compiles [`SET_DISPLAY_MODE_SWIFT`], uploads it to the guest over
/// `/upload`, and `/exec`s it to select a display mode for the `target` logical
/// size at the given backing `scale`:
/// - `scale == 1` — the LoDPI 1× contract (ADR-0014): `target` is the px size
///   (px == pt at 1×) and the helper picks the `px == pt == target` mode. The
///   default path, behaviourally unchanged.
/// - `scale == 2` — the HiDPI/Retina opt-in (ADR-0016 D3): `target` is the
///   *logical* point size and the helper picks the Retina mode whose pixels are
///   2× the points (`pixelWidth == 2·width`), per k4 finding 3.
///
/// **Tolerant throughout**: the switch rides the same optionally-degraded
/// contract as the agent endpoint itself (ADR-0013/0014) — a missing `swiftc`,
/// a compile / upload / exec failure, or a helper that finds no matching mode
/// all warn and return, leaving the VM started at its current resolution. It
/// never fails `vm start`.
///
/// `host`/`port` address the already-ready agent; the caller must only invoke
/// this once the agent has reached health (so this can assume reachability,
/// degrading gracefully if it nonetheless cannot connect).
///
/// Returns `true` only when the switch was confirmed applied (the guest's
/// active mode now reports the target); every degraded path returns `false`,
/// which the HiDPI caller uses to fall back to a 1× switch (ADR-0016).
pub async fn apply(host: &str, port: u16, target: (u32, u32), scale: u32, paths: &VmPaths, id: &str) -> bool {
    let (w, h) = target;
    if scale > 1 {
        eprintln!("Setting guest display to {w}x{h} pt @ {scale}x (HiDPI, {}x{} px)...", w * scale, h * scale);
    } else {
        eprintln!("Setting guest display resolution to {w}x{h} px...");
    }

    let Some(swiftc) = crate::qemu_profile::which("swiftc") else {
        eprintln!("  warning: swiftc not found — skipping resolution switch");
        return false;
    };

    // Stage + host-compile the helper in a per-id scratch dir (cleaned up
    // after), mirroring `golden::provision_wallpaper`.
    let scratch = paths.vms_dir().join(format!("display-{id}"));
    let src = match crate::golden::write_scratch(
        &scratch,
        "set-display-mode.swift",
        SET_DISPLAY_MODE_SWIFT.as_bytes(),
    ) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("  warning: could not stage display helper — skipping ({e})");
            return false;
        }
    };
    let bin = scratch.join("set-display-mode");
    let compiled = std::process::Command::new(swiftc)
        .arg("-o")
        .arg(&bin)
        .arg(&src)
        .output();
    match compiled {
        Ok(out) if out.status.success() => {}
        _ => {
            eprintln!("  warning: could not compile display helper — skipping resolution switch");
            let _ = std::fs::remove_dir_all(&scratch);
            return false;
        }
    }

    let outcome = switch_over_agent(host, port, &bin, w, h, scale).await;
    let applied = outcome.is_ok();
    if let Err(e) = outcome {
        eprintln!("  warning: resolution switch failed (continuing): {e}");
    }
    let _ = std::fs::remove_dir_all(&scratch);
    applied
}

/// Upload the compiled helper over `/upload` and `/exec` it with the px target,
/// returning the agent error on any transport / helper failure. The exec is
/// given a generous timeout: the spike found a brief async framebuffer settle
/// can stall the switch `/exec` for several seconds (up to the agent's exec
/// deadline) while VF reconfigures — that is normal, not an error (ADR-0014
/// Verification finding 4), so the switch must not be aborted underneath it.
async fn switch_over_agent(
    host: &str,
    port: u16,
    bin: &std::path::Path,
    w: u32,
    h: u32,
    scale: u32,
) -> Result<(), AgentError> {
    // Generous exec timeout to absorb the post-switch settle transient; the
    // client adds HTTP headroom past this deadline.
    let config = AgentConfig::new(host, port).with_timeout(Duration::from_secs(60));
    let client = AgentClient::new(config)?;

    let remote = "/tmp/testanyware-set-display-mode";
    client.upload(remote, bin).await?;

    // The helper takes `<logical-w> <logical-h> [scale]`; scale 1 is the LoDPI
    // default (the 2-arg form), scale 2 selects the Retina mode (ADR-0016).
    let command =
        format!("chmod +x {remote} && {remote} {w} {h} {scale}; rc=$?; rm -f {remote}; exit $rc");
    let result = client
        .exec(&ExecRequest { command, timeout: 60, detach: false })
        .await?;
    if result.succeeded() {
        // Surface the helper's confirmation line for the start log.
        let line = result.stdout.trim();
        if !line.is_empty() {
            eprintln!("  {line}");
        }
        Ok(())
    } else {
        // The helper writes a one-line reason to stderr on every non-zero exit.
        let reason = result.stderr.trim();
        let reason = if reason.is_empty() { "helper exited non-zero" } else { reason };
        Err(AgentError::Wire {
            wire_error: "exec_failed".into(),
            details: Some(reason.to_string()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embedded_helper_is_present_and_uses_the_persistent_transaction() {
        // A moved/renamed source would make `include_str!` fail to compile;
        // this guards the *content* contract: the load-bearing transaction
        // call (not the reverted bare `CGDisplaySetDisplayMode`) must be there.
        assert!(SET_DISPLAY_MODE_SWIFT.contains("CGCompleteDisplayConfiguration"));
        assert!(SET_DISPLAY_MODE_SWIFT.contains(".forSession"));
    }

    #[test]
    fn parses_the_default_px_resolution() {
        assert_eq!(parse_target("1920x1080px"), Some((1920, 1080)));
    }

    #[test]
    fn ignores_the_unit_suffix_so_px_pt_and_bare_agree() {
        // All three forms of the same numeric WxH map to the same px target —
        // the 1× contract means px == pt, so the suffix carries no information.
        assert_eq!(parse_target("1920x1080px"), Some((1920, 1080)));
        assert_eq!(parse_target("1920x1080pt"), Some((1920, 1080)));
        assert_eq!(parse_target("1920x1080"), Some((1920, 1080)));
    }

    #[test]
    fn parses_a_non_default_resolution() {
        assert_eq!(parse_target("2560x1440px"), Some((2560, 1440)));
        assert_eq!(parse_target("800x600"), Some((800, 600)));
    }

    #[test]
    fn rejects_malformed_or_zero_dimensions() {
        assert_eq!(parse_target(""), None);
        assert_eq!(parse_target("1920"), None);
        assert_eq!(parse_target("1920x"), None);
        assert_eq!(parse_target("x1080"), None);
        assert_eq!(parse_target("1920x1080xtra"), None);
        assert_eq!(parse_target("widexhigh"), None);
        assert_eq!(parse_target("0x1080"), None);
        assert_eq!(parse_target("1920x0"), None);
    }
}

//! macOS **Setup Assistant** automation over RFB + OCR (ADR-0008) —
//! **macOS-host only**. Takes a freshly IPSW-installed VM (no account yet)
//! to a logged-in desktop with an `admin`/`admin` account, then turns Remote
//! Login on from Terminal. See [`crate::ipsw_base`] for where it fits.
//!
//! ## Shape: a screen table, not a script
//!
//! Setup Assistant's screens vary between macOS releases and with the host's
//! region (an Australian host gets Australia preselected, an extra "Age Range"
//! screen appeared in 27). A fixed keystroke script breaks on the first screen
//! that is added or dropped, so the driver is **table-driven**: [`setup_screens`]
//! lists every screen it knows as an OCR [`Anchor`] plus [`SetupAction`]s; each
//! loop observes the whole screen, finds the first known screen on it
//! ([`match_screen`]), acts, and waits for it to go. An unknown screen is not
//! guessed at: after [`STALL_WAIT`] the run fails with what OCR could read, so
//! a new macOS shows up as a new row to add rather than a mystery.
//!
//! Row **order is specificity**: a sheet over a page (the licence "I have read
//! and agree", FileVault's "Mac Data Will Not Be Securely Encrypted") must
//! precede its page, because the page's anchor is still on screen beneath it.
//!
//! ## macOS 27.0.1 screen sequence (recorded 2026-10-18, display 1024×768)
//!
//! hello splash (greeting cycles languages; **Space** starts it) → Language →
//! Select Your Country or Region → Transfer Your Data to This Mac → Written and
//! Spoken Languages → Accessibility → Data & Privacy → Create a Mac Account
//! (about 30 s of "Creating account…") → Sign In to Your Apple Account (Other
//! Sign-In Options ▸ Sign in Later in Settings ▸ Skip) → Terms and Conditions
//! (Agree ▸ sheet Agree) → Age Range → Choose Your Look → Enable Location
//! Services (left unticked: "Are you sure" sheet ▸ Don't Use, then Select Your
//! Time Zone) → Analytics → Screen Time → Siri → Your Mac is Ready for
//! FileVault (Not Now ▸ sheet Continue) → Update Mac Automatically → Liquid
//! Glass → Welcome (Get Started) → desktop.
//!
//! Notes a later macOS will need: the language and country lists have no
//! keyboard focus until clicked (typing alone does nothing); typing in a list
//! selects by prefix; **Tab** between the account fields replaces the
//! auto-derived account name; the display's pre-guest framebuffer is a black
//! 1280×720 until Setup Assistant draws.

use std::time::{Duration, Instant};

use testanyware_ocr_client::OcrDetection;

use crate::recovery::{deadline_in, round_point, RecoveryError, RecoverySession};

// ---- tunables (live-iteration knobs) ------------------------------------

/// The whole walk-through, hello splash to desktop. A run measured about 5
/// minutes by hand; this bounds a pathological hang.
const TOTAL_WAIT: Duration = Duration::from_secs(25 * 60);
/// No known screen for this long (after the first has been seen) is a stall.
const STALL_WAIT: Duration = Duration::from_secs(2 * 60);
/// How long a label an action wants to click may take to appear.
const LABEL_WAIT: Duration = Duration::from_secs(30);
/// After acting on a screen, how long to wait for it to go away. Account
/// creation is the slow one (about 30 s).
const LEAVE_WAIT: Duration = Duration::from_secs(150);
/// Spacing between observe passes while nothing is recognised.
const IDLE_POLL: Duration = Duration::from_secs(1);
/// The hello splash accepts Space; nudge it this often until a screen is seen.
const NUDGE_EVERY: Duration = Duration::from_secs(8);
/// Pause between actions on one screen (the guest drops back-to-back input).
const ACTION_PAUSE: Duration = Duration::from_millis(500);
/// Terminal needs a moment after Spotlight opens it.
const TERMINAL_WAIT: Duration = Duration::from_secs(60);

// ---- the screen table (pure, unit-tested) -------------------------------

/// How a screen is recognised in the OCR output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Anchor {
    /// Some detection's text contains this (case-insensitive).
    Contains(&'static str),
    /// Some detection's whole text equals this (case-insensitive) — for
    /// words too common to be a substring anchor ("Language").
    Exact(&'static str),
}

impl Anchor {
    fn present(self, detections: &[OcrDetection]) -> bool {
        detections.iter().any(|d| match self {
            Anchor::Contains(s) => d.text.to_lowercase().contains(&s.to_lowercase()),
            Anchor::Exact(s) => d.text.trim().eq_ignore_ascii_case(s),
        })
    }
}

/// One thing to do on a recognised screen.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum SetupAction {
    /// Click the topmost label whose whole text equals this. "Topmost" picks a
    /// sheet's button over the same-named page button beneath it.
    Click(&'static str),
    /// Click the topmost label that merely contains this (a label with a
    /// trailing chevron glyph OCR reads as a stray character).
    ClickContaining(&'static str),
    /// Click `dy` pixels below the centre of the anchor label — a list that
    /// has no text of its own to aim at.
    ClickBelow { anchor: &'static str, dy: f64 },
    /// Type a literal string.
    Type(String),
    /// Press a named key.
    Key(&'static str),
    /// Wait for a label to appear (a menu that opens after a click).
    WaitFor(&'static str),
}

/// A known Setup Assistant screen.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Screen {
    pub name: &'static str,
    pub anchor: Anchor,
    pub actions: Vec<SetupAction>,
    /// Reaching this screen ends the walk-through (the desktop).
    pub done: bool,
}

fn screen(name: &'static str, anchor: Anchor, actions: Vec<SetupAction>) -> Screen {
    Screen { name, anchor, actions, done: false }
}

/// Every Setup Assistant screen macOS 27 is known to show, most specific first.
/// The account created is `user`/`pass`.
pub(crate) fn setup_screens(user: &str, pass: &str) -> Vec<Screen> {
    use Anchor::{Contains, Exact};
    use SetupAction::{Click, ClickBelow, ClickContaining, Key, Type, WaitFor};
    vec![
        // Sheets and in-progress states first: their page's anchor is still
        // on screen beneath them.
        screen("account creation in progress", Contains("Creating account"), vec![]),
        screen("skip Apple Account (confirm)", Contains("Are you sure you want to skip"), vec![
            Click("Skip"),
        ]),
        screen("licence agreement (confirm)", Contains("I have read and agree"), vec![
            Click("Agree"),
        ]),
        screen("FileVault off (confirm)", Contains("Mac Data Will Not Be"), vec![
            Click("Continue"),
        ]),
        // Continue with the box unticked (its default) raises this sheet.
        screen("Location Services off (confirm)", Contains("Are you sure you don't want to"), vec![
            Click("Don't Use"),
        ]),
        // Pages.
        screen("language", Exact("Language"), vec![
            Click("English"),
            // The arrow button has no text; Return activates the default.
            Key("return"),
        ]),
        screen("country or region", Contains("Select Your Country"), vec![
            // The list takes no keyboard focus until clicked; the first row is
            // about 40 px below the heading's centre.
            ClickBelow { anchor: "Select Your Country", dy: 40.0 },
            Type("united states".into()),
            Click("Continue"),
        ]),
        screen("transfer your data", Contains("Transfer Your Data"), vec![
            Click("Set up as new"),
            Click("Continue"),
        ]),
        screen("written and spoken languages", Contains("Written and Spoken Languages"), vec![
            Click("Continue"),
        ]),
        screen("accessibility", Contains("Accessibility features adapt"), vec![Click("Not Now")]),
        screen("data and privacy", Contains("Data & Privacy"), vec![Click("Continue")]),
        screen("create account", Contains("Create a Mac Account"), vec![
            Click("Full Name"),
            Type("Admin".into()),
            // Tab replaces the account name Setup Assistant derived from the
            // full name.
            Key("tab"),
            Type(user.into()),
            Key("tab"),
            Type(pass.into()),
            Key("tab"),
            Type(pass.into()),
            Click("Continue"),
        ]),
        screen("Apple Account sign-in", Contains("Sign In to Your Apple"), vec![
            ClickContaining("Other Sign-In Options"),
            WaitFor("Sign in Later in Settings"),
            ClickContaining("Sign in Later in Settings"),
        ]),
        screen("terms and conditions", Contains("Terms and Conditions"), vec![Click("Agree")]),
        screen("age range", Contains("Age Range"), vec![Click("Adult")]),
        screen("appearance", Contains("Choose Your Look"), vec![Click("Light"), Click("Continue")]),
        screen("location services", Contains("Enable Location Services"), vec![Click("Continue")]),
        // Shown only when Location Services is off (nothing to infer it from).
        screen("time zone", Contains("Select Your Time Zone"), vec![Click("Continue")]),
        screen("analytics", Contains("Share Mac Analytics"), vec![Click("Continue")]),
        screen("screen time", Contains("Downtime and App Limits"), vec![Click("Set Up Later")]),
        screen("Siri", Contains("Find information and get things done"), vec![
            Click("Set Up Later"),
        ]),
        screen("FileVault", Contains("Ready for FileVault"), vec![Click("Not Now")]),
        screen("software update", Contains("Update Mac Automatically"), vec![
            Click("Only Download Automatically"),
        ]),
        screen("Liquid Glass", Contains("Liquid Glass"), vec![Click("Continue")]),
        screen("welcome", Contains("Get Started"), vec![Click("Get Started")]),
        Screen { name: "desktop", anchor: Contains("Finder"), actions: vec![], done: true },
    ]
}

/// Index of the first screen in `screens` whose anchor is on screen.
pub(crate) fn match_screen(screens: &[Screen], detections: &[OcrDetection]) -> Option<usize> {
    screens.iter().position(|s| s.anchor.present(detections))
}

/// The topmost detection satisfying `pred`.
fn topmost(
    detections: &[OcrDetection],
    pred: impl Fn(&OcrDetection) -> bool,
) -> Option<&OcrDetection> {
    detections
        .iter()
        .filter(|d| pred(d))
        .min_by(|a, b| a.y.partial_cmp(&b.y).unwrap_or(std::cmp::Ordering::Equal))
}

fn exact_label<'a>(detections: &'a [OcrDetection], text: &str) -> Option<&'a OcrDetection> {
    topmost(detections, |d| d.text.trim().eq_ignore_ascii_case(text))
}

fn containing_label<'a>(detections: &'a [OcrDetection], text: &str) -> Option<&'a OcrDetection> {
    let needle = text.to_lowercase();
    topmost(detections, |d| d.text.to_lowercase().contains(&needle))
}

// ---- the driver (verified live, not unit-tested) ------------------------

fn stalled(what: &str, detections: &[OcrDetection]) -> RecoveryError {
    let seen: Vec<&str> = detections.iter().map(|d| d.text.as_str()).collect();
    RecoveryError::Ocr(format!("Setup Assistant {what}; OCR read: {seen:?}"))
}

async fn wait_for_label(
    s: &mut RecoverySession,
    find: impl Fn(&[OcrDetection]) -> Option<(f64, f64)>,
    what: &str,
) -> Result<(f64, f64), RecoveryError> {
    let deadline = deadline_in(LABEL_WAIT);
    loop {
        let dets = s.observe().await?;
        if let Some(point) = find(&dets) {
            return Ok(point);
        }
        if Instant::now() >= deadline {
            return Err(stalled(&format!("never showed {what}"), &dets));
        }
        tokio::time::sleep(IDLE_POLL).await;
    }
}

async fn run_action(s: &mut RecoverySession, action: &SetupAction) -> Result<(), RecoveryError> {
    match action {
        SetupAction::Click(text) => {
            let c = wait_for_label(s, |d| exact_label(d, text).map(|x| x.centre()), text).await?;
            let (x, y) = round_point(c);
            s.click_at(x, y).await?;
        }
        SetupAction::ClickContaining(text) => {
            let c =
                wait_for_label(s, |d| containing_label(d, text).map(|x| x.centre()), text).await?;
            let (x, y) = round_point(c);
            s.click_at(x, y).await?;
        }
        SetupAction::ClickBelow { anchor, dy } => {
            let c = wait_for_label(s, |d| containing_label(d, anchor).map(|x| x.centre()), anchor)
                .await?;
            let (x, y) = round_point((c.0, c.1 + dy));
            s.click_at(x, y).await?;
        }
        SetupAction::Type(text) => s.type_text(text).await?,
        SetupAction::Key(name) => s.key(name).await?,
        SetupAction::WaitFor(text) => {
            wait_for_label(s, |d| containing_label(d, text).map(|x| x.centre()), text).await?;
        }
    }
    tokio::time::sleep(ACTION_PAUSE).await;
    Ok(())
}

/// Walk Setup Assistant to the desktop, creating the `user`/`pass` account.
pub(crate) async fn run_setup_assistant(
    s: &mut RecoverySession,
    user: &str,
    pass: &str,
) -> Result<(), RecoveryError> {
    let screens = setup_screens(user, pass);
    let started = Instant::now();
    let mut last_progress = Instant::now();
    let mut last_nudge: Option<Instant> = None;
    let mut seen_any = false;

    loop {
        let dets = s.observe().await?;
        let Some(idx) = match_screen(&screens, &dets) else {
            if started.elapsed() > TOTAL_WAIT {
                return Err(stalled("did not finish in time", &dets));
            }
            if seen_any && last_progress.elapsed() > STALL_WAIT {
                return Err(stalled("stalled on an unknown screen", &dets));
            }
            // Before any known screen, the hello splash is waiting for Space.
            if !seen_any && last_nudge.map_or(true, |t| t.elapsed() >= NUDGE_EVERY) {
                s.key("space").await?;
                last_nudge = Some(Instant::now());
            }
            tokio::time::sleep(IDLE_POLL).await;
            continue;
        };
        seen_any = true;
        last_progress = Instant::now();
        let current = &screens[idx];
        eprintln!("  Setup Assistant: {}", current.name);
        if current.done {
            return Ok(());
        }
        for action in &current.actions {
            run_action(s, action).await?;
        }
        // Wait for this screen to go before re-observing, so a slow
        // transition does not make the same actions run twice.
        let leave = deadline_in(LEAVE_WAIT);
        loop {
            tokio::time::sleep(IDLE_POLL).await;
            let dets = s.observe().await?;
            if match_screen(&screens, &dets) != Some(idx) || Instant::now() >= leave {
                break;
            }
        }
    }
}

/// Turn Remote Login on from the desktop: open Terminal through Spotlight and
/// load the `sshd` launch daemon (`sudo`, answering the password prompt).
/// `systemsetup -setremotelogin` is not used: it needs Full Disk Access, which
/// a fresh Terminal does not have.
pub(crate) async fn enable_remote_login(
    s: &mut RecoverySession,
    pass: &str,
) -> Result<(), RecoveryError> {
    eprintln!("  Opening Terminal (Spotlight)...");
    s.key_chord("space", &["cmd"]).await?;
    // Let the Spotlight field take focus before typing into it.
    tokio::time::sleep(Duration::from_secs(2)).await;
    s.type_text("Terminal").await?;
    tokio::time::sleep(Duration::from_secs(2)).await;
    s.key("return").await?;
    // The shell prompt's "Last login" line shows once Terminal is ready.
    let deadline = deadline_in(TERMINAL_WAIT);
    s.wait_for_text("Last login", deadline).await?;
    tokio::time::sleep(ACTION_PAUSE).await;

    eprintln!("  Enabling Remote Login...");
    s.type_text("sudo launchctl load -w /System/Library/LaunchDaemons/ssh.plist").await?;
    s.key("return").await?;
    s.wait_for_text("Password", deadline_in(Duration::from_secs(30))).await?;
    s.type_text(pass).await?;
    s.key("return").await?;
    s.settle(deadline_in(Duration::from_secs(20))).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn det(text: &str, y: f64) -> OcrDetection {
        OcrDetection::new(text, 100.0, y, 80.0, 14.0, 1.0)
    }

    fn dets(lines: &[(&str, f64)]) -> Vec<OcrDetection> {
        lines.iter().map(|(t, y)| det(t, *y)).collect()
    }

    fn name_of(screens: &[Screen], dets: &[OcrDetection]) -> Option<&'static str> {
        match_screen(screens, dets).map(|i| screens[i].name)
    }

    // The OCR lines below are copied from the screens of a live macOS 27.0.1
    // walk-through (2026-10-18), so the table is pinned against what OCR
    // actually returned, not what a screen is assumed to say.

    #[test]
    fn recognises_each_recorded_screen() {
        let s = setup_screens("admin", "admin");
        let cases: &[(&[(&str, f64)], &str)] = &[
            (&[("Language", 269.0), ("English (Australia)", 320.0), ("English", 370.0)], "language"),
            (&[("Select Your Country or Region", 260.0), ("Australia", 301.0)], "country or region"),
            (&[("Transfer Your Data to This Mac", 272.0), ("Set up as new", 550.0)], "transfer your data"),
            (&[("Written and Spoken Languages", 272.0), ("Preferred Languages", 374.0)], "written and spoken languages"),
            (&[("Accessibility", 274.0), ("Accessibility features adapt your Mac to your individual", 291.0), ("Not Now", 640.0)], "accessibility"),
            (&[("Data & Privacy", 260.0), ("Continue", 640.0)], "data and privacy"),
            (&[("Create a Mac Account", 152.0), ("Full Name", 336.0), ("Continue", 640.0)], "create account"),
            (&[("Sign In to Your Apple Account", 269.0), ("Other Sign-In Options v", 640.0)], "Apple Account sign-in"),
            (&[("Age Range", 274.0), ("Adult", 546.0)], "age range"),
            (&[("Choose Your Look", 150.0), ("Light", 492.0)], "appearance"),
            (&[("Enable Location Services", 272.0), ("Enable Location Services on this Mac", 400.0)], "location services"),
            (&[("Select Your Time Zone", 260.0), ("Cupertino, CA - United States", 480.0), ("Continue", 640.0)], "time zone"),
            (&[("Analytics", 272.0), ("Share Mac Analytics with Apple", 360.0)], "analytics"),
            (&[("Screen Time", 152.0), ("Downtime and App Limits", 340.0), ("Set Up Later", 639.0)], "screen time"),
            (&[("Siri", 380.0), ("Find information and get things done, simply by asking.", 400.0)], "Siri"),
            (&[("Your Mac is Ready for FileVault", 272.0), ("Not Now", 642.0)], "FileVault"),
            (&[("Update Mac Automatically", 274.0), ("Only Download Automatically", 640.0)], "software update"),
            (&[("Liquid Glass", 150.0), ("Accessibility", 642.0)], "Liquid Glass"),
            (&[("welcome", 319.0), ("Get Started", 644.0)], "welcome"),
            (&[("Finder", 10.0), ("File", 8.0), ("Edit", 8.0), ("Sun Oct 18 3:36 AM", 8.0)], "desktop"),
        ];
        for (lines, want) in cases {
            assert_eq!(name_of(&s, &dets(lines)), Some(*want), "{lines:?}");
        }
    }

    #[test]
    fn a_sheet_wins_over_the_page_beneath_it() {
        let s = setup_screens("admin", "admin");
        // Licence sheet: the page heading is still on screen under it.
        let licence = dets(&[
            ("Terms and Conditions", 150.0),
            ("I have read and agree to the", 382.0),
            ("Agree", 454.0),
            ("Agree", 642.0),
        ]);
        assert_eq!(name_of(&s, &licence), Some("licence agreement (confirm)"));
        let skip = dets(&[
            ("Sign In to Your Apple Account", 270.0),
            ("Are you sure you want to skip", 354.0),
            ("Skip", 482.0),
        ]);
        assert_eq!(name_of(&s, &skip), Some("skip Apple Account (confirm)"));
        // Recorded live; the page's own heading was garbled by the sheet's blur.
        let location = dets(&[
            ("Enable Location Carvicac", 272.0),
            ("Are you sure you don't want to", 354.0),
            ("Don't Use", 440.0),
            ("Use Location Services", 440.0),
            ("Continue", 640.0),
        ]);
        assert_eq!(name_of(&s, &location), Some("Location Services off (confirm)"));
        let filevault = dets(&[
            ("Your Mac is Ready for FileVault", 272.0),
            ("Mac Data Will Not Be", 314.0),
            ("Continue", 440.0),
        ]);
        assert_eq!(name_of(&s, &filevault), Some("FileVault off (confirm)"));
        // And account creation in progress must not re-run the form.
        let creating = dets(&[("Create a Mac Account", 152.0), ("Creating account...", 638.0)]);
        assert_eq!(name_of(&s, &creating), Some("account creation in progress"));
    }

    #[test]
    fn unknown_or_empty_screens_match_nothing() {
        let s = setup_screens("admin", "admin");
        assert_eq!(match_screen(&s, &[]), None);
        // The hello splash: a greeting in some language and its Continue.
        assert_eq!(match_screen(&s, &dets(&[("hallo", 320.0), ("Fortfahren", 644.0)])), None);
    }

    #[test]
    fn exact_anchor_does_not_fire_on_longer_text_containing_it() {
        let s = setup_screens("admin", "admin");
        // "Language" appears inside many other screens' text; only a whole
        // "Language" heading is the language screen.
        let other = dets(&[("Preferred Languages", 374.0), ("Input Sources", 431.0)]);
        assert_ne!(name_of(&s, &other), Some("language"));
    }

    #[test]
    fn click_prefers_exact_text_and_the_topmost_match() {
        // "Agree" must not hit "Disagree" or the licence prose, and the
        // sheet's button (higher on screen) beats the page's button.
        let d = dets(&[
            ("Disagree", 451.0),
            ("Agree", 454.0),
            ("YOU AGREE TO BE BOUND", 440.0),
            ("Disagree", 642.0),
            ("Agree", 642.0),
        ]);
        let hit = exact_label(&d, "Agree").unwrap();
        assert_eq!(hit.y, 454.0);
        assert!(exact_label(&d, "Agre").is_none());
    }

    #[test]
    fn contains_click_finds_a_label_with_a_trailing_glyph() {
        let d = dets(&[("Other Sign-In Options v", 640.0), ("Continue", 640.0)]);
        assert!(exact_label(&d, "Other Sign-In Options").is_none());
        assert!(containing_label(&d, "Other Sign-In Options").is_some());
    }

    #[test]
    fn account_form_types_the_requested_credentials() {
        let s = setup_screens("builder", "s3cret");
        let form = s.iter().find(|s| s.name == "create account").unwrap();
        let typed: Vec<&str> = form
            .actions
            .iter()
            .filter_map(|a| match a {
                SetupAction::Type(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        // Full name, account name, password, verify password.
        assert_eq!(typed, ["Admin", "builder", "s3cret", "s3cret"]);
    }

    #[test]
    fn exactly_one_screen_ends_the_walk_and_it_is_last() {
        let s = setup_screens("admin", "admin");
        assert_eq!(s.iter().filter(|x| x.done).count(), 1);
        assert!(s.last().unwrap().done);
        // Every other screen has something to do, except the in-progress wait.
        for x in s.iter().filter(|x| !x.done && x.name != "account creation in progress") {
            assert!(!x.actions.is_empty(), "{} has no actions", x.name);
        }
    }
}

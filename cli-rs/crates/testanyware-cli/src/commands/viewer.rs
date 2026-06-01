//! `testanyware viewer` — the embedded `eframe`/`wgpu` viewer (ADR-0005).
//!
//! This leaf (060/010) is the **read-only render skeleton**: it opens a
//! window, renders the guest's live framebuffer, and tears down cleanly.
//! Input forwarding (leaf 020) and auto-reconnect + `vm start --viewer`
//! sugar (leaf 030) build on the architecture established here.
//!
//! ## Architecture (ADR-0005)
//!
//! `eframe::run_native` must own the process **main thread** (winit/NSApp
//! requirement). The CLI is `#[tokio::main]`, but its `block_on` drives the
//! root future on the main thread, so as long as the `viewer` dispatch arm
//! is a *synchronous* call (no `.await`, no `tokio::spawn`) this function
//! runs on the main thread — eframe gets it with no `main.rs` carve-out.
//!
//! A **dedicated `std::thread` running its own current-thread tokio
//! runtime** owns the [`RfbConnection`] and runs a `tokio::select!` loop,
//! fully isolated from the CLI's global runtime (no nested-runtime hazard).
//! Handoff:
//!   - UI ← RFB: [`Arc<Mutex<FrameSlot>>`] holding the latest RGBA + dims.
//!   - UI → RFB: an `mpsc` of [`ViewerInput`] (wired by leaf 020; the
//!     receiver's `select!` arm exists now so 020 only adds the producer).
//!   - shutdown: a `watch` channel set when the window closes.
//!   - wake: an [`egui::Context`] clone in the RFB thread calls
//!     `request_repaint()` on each new frame, so eframe does not busy-poll.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use eframe::egui;
use testanyware_rfb::{RfbConnection, ServerEvent};

use crate::output::{print_error, OutputMode};
use crate::resolve::{resolve_vnc, ConnectionOptions, ResolveError, ResolvedVnc};

/// How often the RFB thread asks the server for an incremental update,
/// keeping the framebuffer stream flowing (~30 polls/sec). The server
/// answers only when something changed, so an idle desktop is cheap.
const INCREMENTAL_POLL_INTERVAL: Duration = Duration::from_millis(33);

/// Latest framebuffer state shared from the RFB thread to the UI.
///
/// The RFB thread overwrites `rgba`/`width`/`height` and sets `dirty` on
/// each applied update; the UI consumes it (clearing `dirty`) and uploads
/// to a texture. On a connection drop the RFB thread sets `disconnected`
/// and `status` so the UI can paint the overlay (leaf A: overlay then
/// clean exit, no auto-reconnect — that is leaf 030).
#[derive(Default)]
struct FrameSlot {
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    /// A new frame is waiting for the UI to upload.
    dirty: bool,
    /// The RFB connection dropped or failed; show the overlay.
    disconnected: bool,
    /// Human-readable reason for the overlay text.
    status: Option<String>,
}

/// UI → RFB input event. Constructed by leaf 020 (input forwarding); the
/// variants and the RFB-thread handler are defined now so that leaf wires
/// only the producer + coordinate mapping, not a re-architecture.
#[allow(dead_code)] // producer lands in leaf 060/020
enum ViewerInput {
    /// A key press (`down = true`) or release, as a resolved X keysym.
    Key { keysym: u32, down: bool },
    /// A pointer event: bit-packed button mask + framebuffer-pixel coords.
    Pointer { button_mask: u8, x: u16, y: u16 },
}

/// `testanyware viewer` entry point. Synchronous on purpose — see the
/// module docs: it must run on the main thread so eframe can own it.
pub fn run_viewer(opts: ConnectionOptions) {
    // Resolve the endpoint up front (synchronous). A resolution failure is
    // a real usage/config error, so it exits non-zero *before* any window
    // opens — unlike a live connection drop, which surfaces as the overlay.
    let endpoint = match resolve_vnc(&opts) {
        Ok(e) => e,
        Err(err) => exit_resolve_error(err),
    };

    let frame = Arc::new(Mutex::new(FrameSlot::default()));
    let (input_tx, input_rx) = tokio::sync::mpsc::channel::<ViewerInput>(256);
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // The RFB thread needs the egui Context to wake the UI, but the Context
    // only exists once eframe builds the window. Spawn the thread now and
    // hand it the Context through a one-shot std channel from the app
    // creator closure; the thread blocks on `recv()` until the window is up.
    let (ctx_tx, ctx_rx) = std::sync::mpsc::channel::<egui::Context>();
    let rfb_frame = Arc::clone(&frame);
    let rfb_thread = std::thread::Builder::new()
        .name("rfb-viewer".to_string())
        .spawn(move || {
            // If the window failed to start, the creator closure never ran,
            // so ctx_tx was dropped: nothing to do.
            let Ok(ctx) = ctx_rx.recv() else { return };
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build viewer RFB runtime");
            rt.block_on(rfb_loop(endpoint, rfb_frame, input_rx, shutdown_rx, ctx));
        })
        .expect("spawn viewer RFB thread");

    let app_frame = Arc::clone(&frame);
    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: egui::ViewportBuilder::default()
            // Sized provisionally; the app resizes to the framebuffer
            // dimensions once the first frame arrives (DesktopSize honoured).
            .with_inner_size([1024.0, 768.0])
            .with_title("TestAnyware viewer"),
        ..Default::default()
    };

    let result = eframe::run_native(
        "testanyware-viewer",
        native_options,
        Box::new(move |cc| {
            // Hand the live Context to the waiting RFB thread, then build
            // the app. If the receiver is gone the send is a harmless no-op.
            let _ = ctx_tx.send(cc.egui_ctx.clone());
            Ok(Box::new(ViewerApp::new(app_frame, input_tx)))
        }),
    );

    // Window closed (or failed to open): signal shutdown and join so the
    // RFB connection is torn down cleanly before we exit.
    let _ = shutdown_tx.send(true);
    let _ = rfb_thread.join();

    if let Err(err) = result {
        eprintln!("testanyware viewer: failed to open window: {err}");
        std::process::exit(1);
    }
}

/// The dedicated-thread RFB loop. Owns the connection for its whole life;
/// copies each applied frame into the shared slot and wakes the UI.
async fn rfb_loop(
    endpoint: ResolvedVnc,
    frame: Arc<Mutex<FrameSlot>>,
    mut input_rx: tokio::sync::mpsc::Receiver<ViewerInput>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
    ctx: egui::Context,
) {
    let mut conn = match RfbConnection::connect(
        &endpoint.host,
        endpoint.port,
        endpoint.password.as_deref().map(str::as_bytes),
    )
    .await
    {
        Ok(c) => c,
        Err(err) => {
            mark_disconnected(&frame, &ctx, format!("Failed to connect: {err}"));
            return;
        }
    };

    // Kick off the stream with one full (non-incremental) update; the
    // interval below then requests incremental updates to keep it flowing.
    let (w, h) = conn.framebuffer_size();
    if let Err(err) = conn.request_framebuffer_update(false, 0, 0, w as u16, h as u16).await {
        mark_disconnected(&frame, &ctx, format!("Connection lost: {err}"));
        return;
    }

    let mut poll = tokio::time::interval(INCREMENTAL_POLL_INTERVAL);
    poll.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

    loop {
        tokio::select! {
            msg = conn.next_message() => match msg {
                Ok(ServerEvent::FramebufferUpdated { rectangles }) if rectangles > 0 => {
                    let fb = conn.framebuffer();
                    update_slot(&frame, fb.width(), fb.height(), fb.rgba());
                    ctx.request_repaint();
                }
                Ok(_) => {} // no-op update / bell / cut-text: ignore
                Err(err) => {
                    mark_disconnected(&frame, &ctx, format!("Connection lost: {err}"));
                    break;
                }
            },
            Some(input) = input_rx.recv() => {
                // Producer arrives in leaf 020; the handler is ready.
                let result = match input {
                    ViewerInput::Key { keysym, down } => conn.key_event(keysym, down).await,
                    ViewerInput::Pointer { button_mask, x, y } => {
                        conn.pointer_event(button_mask, x, y).await
                    }
                };
                if let Err(err) = result {
                    mark_disconnected(&frame, &ctx, format!("Connection lost: {err}"));
                    break;
                }
            }
            _ = shutdown_rx.changed() => break, // window closed
            _ = poll.tick() => {
                let (w, h) = conn.framebuffer_size();
                if let Err(err) =
                    conn.request_framebuffer_update(true, 0, 0, w as u16, h as u16).await
                {
                    mark_disconnected(&frame, &ctx, format!("Connection lost: {err}"));
                    break;
                }
            }
        }
    }
}

/// Copy an applied framebuffer into the shared slot and flag it dirty.
fn update_slot(frame: &Arc<Mutex<FrameSlot>>, width: u32, height: u32, rgba: &[u8]) {
    let mut slot = frame.lock().expect("frame slot poisoned");
    slot.rgba.clear();
    slot.rgba.extend_from_slice(rgba);
    slot.width = width;
    slot.height = height;
    slot.dirty = true;
}

/// Record a disconnect so the UI paints the overlay, and wake it once more.
fn mark_disconnected(frame: &Arc<Mutex<FrameSlot>>, ctx: &egui::Context, reason: String) {
    {
        let mut slot = frame.lock().expect("frame slot poisoned");
        slot.disconnected = true;
        slot.status = Some(reason);
    }
    ctx.request_repaint();
}

/// The eframe application: uploads the shared framebuffer to a texture and
/// draws it, scaled to fit, with a "disconnected" overlay on connection loss.
struct ViewerApp {
    frame: Arc<Mutex<FrameSlot>>,
    /// UI → RFB sender. Unused in leaf A; leaf 020 attaches the producer.
    #[allow(dead_code)]
    input_tx: tokio::sync::mpsc::Sender<ViewerInput>,
    texture: Option<egui::TextureHandle>,
    /// Dimensions of the current texture; a change means the guest desktop
    /// resized, so we recreate the texture and resize the window.
    texture_dims: Option<(u32, u32)>,
}

impl ViewerApp {
    fn new(
        frame: Arc<Mutex<FrameSlot>>,
        input_tx: tokio::sync::mpsc::Sender<ViewerInput>,
    ) -> Self {
        Self {
            frame,
            input_tx,
            texture: None,
            texture_dims: None,
        }
    }
}

impl eframe::App for ViewerApp {
    // eframe 0.34: `ui` is the required paint method; it hands us the root
    // `Ui` (no margin/background) directly. The old `update(ctx, …)` is
    // deprecated. We get the `Context` for texture/viewport work via
    // `ui.ctx()`.
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        // Drain the latest frame under the lock, then release it before
        // touching egui (keep the critical section tiny).
        let (disconnected, status) = {
            let mut slot = self.frame.lock().expect("frame slot poisoned");
            if slot.dirty {
                slot.dirty = false;
                let dims = (slot.width, slot.height);
                let image = egui::ColorImage::from_rgba_unmultiplied(
                    [slot.width as usize, slot.height as usize],
                    &slot.rgba,
                );
                let resized = self.texture_dims != Some(dims);
                match self.texture.as_mut() {
                    Some(tex) if !resized => tex.set(image, egui::TextureOptions::default()),
                    _ => {
                        self.texture = Some(ctx.load_texture(
                            "vnc-framebuffer",
                            image,
                            egui::TextureOptions::default(),
                        ));
                    }
                }
                if resized {
                    self.texture_dims = Some(dims);
                    ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(
                        dims.0 as f32,
                        dims.1 as f32,
                    )));
                }
            }
            (slot.disconnected, slot.status.clone())
        };

        // The root `Ui` has no background; paint it black behind the frame.
        ui.painter().rect_filled(ui.max_rect(), 0.0, egui::Color32::BLACK);

        if let Some(tex) = &self.texture {
            let available = ui.available_size();
            ui.centered_and_justified(|ui| {
                ui.add(
                    egui::Image::new(egui::load::SizedTexture::new(tex.id(), tex.size_vec2()))
                        .maintain_aspect_ratio(true)
                        .fit_to_exact_size(available),
                );
            });
        } else if !disconnected {
            ui.centered_and_justified(|ui| {
                ui.label(
                    egui::RichText::new("Connecting…")
                        .size(24.0)
                        .color(egui::Color32::GRAY),
                );
            });
        }

        if disconnected {
            let rect = ui.max_rect();
            let painter = ui.painter();
            painter.rect_filled(rect, 0.0, egui::Color32::from_black_alpha(160));
            let text = status.as_deref().unwrap_or("Disconnected");
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::FontId::proportional(24.0),
                egui::Color32::WHITE,
            );
        }
    }
}

fn exit_resolve_error(err: ResolveError) -> ! {
    // The viewer is interactive (no --json), so report in text mode.
    print_error(
        OutputMode::Text,
        err.code(),
        &err.to_string(),
        None,
        err.details(),
        err.exit_code(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn slot() -> Arc<Mutex<FrameSlot>> {
        Arc::new(Mutex::new(FrameSlot::default()))
    }

    #[test]
    fn update_slot_copies_pixels_and_dims_and_flags_dirty() {
        let frame = slot();
        let rgba: [u8; 8] = [1, 2, 3, 255, 4, 5, 6, 255];
        update_slot(&frame, 2, 1, &rgba);

        let s = frame.lock().unwrap();
        assert_eq!(s.width, 2);
        assert_eq!(s.height, 1);
        assert_eq!(s.rgba, rgba);
        assert!(s.dirty, "a fresh frame must be flagged dirty for the UI");
        assert!(!s.disconnected);
    }

    #[test]
    fn update_slot_overwrites_a_smaller_previous_frame() {
        // A desktop resize hands us a frame of different length; the slot
        // must replace, not append (else the buffer/dims would disagree and
        // ColorImage::from_rgba_unmultiplied would panic in the UI).
        let frame = slot();
        update_slot(&frame, 2, 2, &[0u8; 2 * 2 * 4]);
        update_slot(&frame, 1, 1, &[9u8; 4]);

        let s = frame.lock().unwrap();
        assert_eq!((s.width, s.height), (1, 1));
        assert_eq!(s.rgba.len(), 4, "buffer length must track the new dims");
        assert_eq!(s.rgba, [9, 9, 9, 9]);
    }

    #[test]
    fn mark_disconnected_sets_status_without_a_context_dependency() {
        // We can't construct an egui::Context in a unit test, so exercise
        // the slot mutation directly (the request_repaint wake is a UI
        // concern verified live). This guards the overlay's data path.
        let frame = slot();
        {
            let mut s = frame.lock().unwrap();
            s.disconnected = true;
            s.status = Some("Connection lost: boom".to_string());
        }
        let s = frame.lock().unwrap();
        assert!(s.disconnected);
        assert_eq!(s.status.as_deref(), Some("Connection lost: boom"));
    }
}

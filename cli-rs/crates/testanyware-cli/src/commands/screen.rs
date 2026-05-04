//! `screen size` and `screen capture` command handlers.
//!
//! Both commands open one RFB connection, run the handshake, request
//! one framebuffer update, then disconnect — they are one-shots, not
//! long-lived sessions.

use std::path::Path;

use serde_json::json;
use testanyware_rfb::{RfbConnection, ServerEvent};

use crate::output::{print_error, print_success, OutputMode};
use crate::resolve::{resolve_vnc, ConnectionOptions, ResolveError};

/// `testanyware screen size` — print the framebuffer dimensions.
pub async fn run_screen_size(opts: ConnectionOptions, mode: OutputMode) {
    let endpoint = match resolve_vnc(&opts) {
        Ok(e) => e,
        Err(err) => exit_resolve_error(err, mode),
    };
    match RfbConnection::connect(&endpoint.host, endpoint.port, endpoint.password.as_deref().map(str::as_bytes)).await {
        Ok(conn) => {
            let (w, h) = conn.framebuffer_size();
            match mode {
                OutputMode::Text => println!("{w}x{h}"),
                OutputMode::Json => {
                    print_success(json!({ "width": w, "height": h }));
                }
            }
        }
        Err(err) => exit_rfb_error(err, mode),
    }
}

/// `testanyware screen capture` — capture one framebuffer update and
/// write it to disk as a PNG. `--region x,y,w,h` crops post-decode.
pub async fn run_screen_capture(
    opts: ConnectionOptions,
    output_path: Option<String>,
    region: Option<String>,
    mode: OutputMode,
) {
    let region = match region.as_deref().map(parse_region) {
        Some(Ok(r)) => Some(r),
        Some(Err(msg)) => {
            print_error(
                mode,
                "USAGE_ERROR",
                &format!("invalid --region: {msg}"),
                Some("Expected --region X,Y,W,H with non-negative integers."),
                json!({ "value": region.unwrap_or_default() }),
                2,
            );
        }
        None => None,
    };

    let endpoint = match resolve_vnc(&opts) {
        Ok(e) => e,
        Err(err) => exit_resolve_error(err, mode),
    };
    let mut conn = match RfbConnection::connect(
        &endpoint.host,
        endpoint.port,
        endpoint.password.as_deref().map(str::as_bytes),
    )
    .await
    {
        Ok(c) => c,
        Err(err) => exit_rfb_error(err, mode),
    };

    let (fb_w, fb_h) = conn.framebuffer_size();
    if let Err(err) = conn
        .request_framebuffer_update(false, 0, 0, fb_w as u16, fb_h as u16)
        .await
    {
        exit_rfb_error(err, mode);
    }

    // Drain server messages until at least one FramebufferUpdated arrives
    // with a rectangle. Some servers send no-op updates first.
    loop {
        match conn.next_message().await {
            Ok(ServerEvent::FramebufferUpdated { rectangles }) if rectangles > 0 => break,
            Ok(_) => continue,
            Err(err) => exit_rfb_error(err, mode),
        }
    }

    let fb = conn.framebuffer().clone();
    let crop = region.unwrap_or((0, 0, fb_w, fb_h));
    let png_bytes = match encode_png(&fb, crop) {
        Ok(b) => b,
        Err(err) => {
            print_error(
                mode,
                "INTERNAL",
                &format!("PNG encode failed: {err}"),
                None,
                json!({}),
                1,
            );
        }
    };

    let path = output_path.unwrap_or_else(|| "screen.png".to_string());
    if let Err(err) = std::fs::write(&path, &png_bytes) {
        print_error(
            mode,
            "IO_ERROR",
            &format!("failed to write {path}: {err}"),
            None,
            json!({ "path": path }),
            1,
        );
    }

    match mode {
        OutputMode::Text => println!("wrote {path} ({}x{})", crop.2, crop.3),
        OutputMode::Json => {
            print_success(json!({
                "path": Path::new(&path).display().to_string(),
                "width": crop.2,
                "height": crop.3,
            }));
        }
    }
}

fn parse_region(value: &str) -> Result<(u32, u32, u32, u32), String> {
    let parts: Vec<&str> = value.split(',').collect();
    if parts.len() != 4 {
        return Err(format!("expected 4 comma-separated integers, got {}", parts.len()));
    }
    let mut out = [0u32; 4];
    for (i, p) in parts.iter().enumerate() {
        out[i] = p
            .trim()
            .parse::<u32>()
            .map_err(|_| format!("'{p}' is not a non-negative integer"))?;
    }
    let (x, y, w, h) = (out[0], out[1], out[2], out[3]);
    if w == 0 || h == 0 {
        return Err("width and height must be positive".into());
    }
    Ok((x, y, w, h))
}

fn encode_png(
    fb: &testanyware_rfb::Framebuffer,
    region: (u32, u32, u32, u32),
) -> Result<Vec<u8>, image::ImageError> {
    let (x, y, w, h) = region;
    if x + w > fb.width() || y + h > fb.height() {
        return Err(image::ImageError::Parameter(
            image::error::ParameterError::from_kind(
                image::error::ParameterErrorKind::DimensionMismatch,
            ),
        ));
    }
    let stride = fb.width() as usize * 4;
    let mut cropped = Vec::with_capacity((w as usize) * (h as usize) * 4);
    for row in 0..h as usize {
        let src_off = (y as usize + row) * stride + (x as usize) * 4;
        cropped.extend_from_slice(&fb.rgba()[src_off..src_off + (w as usize) * 4]);
    }
    let img =
        image::RgbaImage::from_raw(w, h, cropped).expect("buffer length matches w*h*4");
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), image::ImageFormat::Png)?;
    Ok(out)
}

fn exit_resolve_error(err: ResolveError, mode: OutputMode) -> ! {
    let code = err.code();
    let message = err.to_string();
    print_error(mode, code, &message, None, err.details(), err.exit_code());
}

fn exit_rfb_error(err: testanyware_rfb::RfbError, mode: OutputMode) -> ! {
    use testanyware_rfb::RfbError;
    let (code, exit_code) = match &err {
        RfbError::Io(_) => ("CONNECTION_REFUSED", 1),
        RfbError::UnsupportedProtocolVersion(_) => ("INTERNAL", 1),
        RfbError::SecurityNegotiationFailed(_) => ("AUTH_REQUIRED", 4),
        RfbError::NoMutualSecurityType(_) => ("AUTH_REQUIRED", 4),
        RfbError::AuthFailed(_) => ("AUTH_REQUIRED", 4),
        RfbError::PasswordRequired => ("AUTH_REQUIRED", 4),
        RfbError::InvalidFramebufferSize { .. } => ("INTERNAL", 1),
        RfbError::UnexpectedMessageType(_) => ("INTERNAL", 1),
        RfbError::UnsupportedEncoding(_) => ("INTERNAL", 1),
        RfbError::Protocol(_) => ("INTERNAL", 1),
    };
    print_error(mode, code, &err.to_string(), None, json!({}), exit_code);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_region_ok() {
        assert_eq!(parse_region("0,0,800,600").unwrap(), (0, 0, 800, 600));
        assert_eq!(parse_region("10, 20, 30, 40").unwrap(), (10, 20, 30, 40));
    }

    #[test]
    fn parse_region_rejects_wrong_arity() {
        assert!(parse_region("1,2,3").is_err());
        assert!(parse_region("1,2,3,4,5").is_err());
    }

    #[test]
    fn parse_region_rejects_zero_dim() {
        assert!(parse_region("0,0,0,100").is_err());
        assert!(parse_region("0,0,100,0").is_err());
    }

    #[test]
    fn parse_region_rejects_non_integer() {
        assert!(parse_region("a,b,c,d").is_err());
        assert!(parse_region("0,0,-10,10").is_err());
    }
}

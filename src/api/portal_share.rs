//! XDG portal screen-cast picker (feature `portal`).
//!
//! On Wayland `ximagesrc` cannot capture the screen; the compositor must
//! grant a PipeWire stream through a user-approved dialog. This module runs
//! that flow and hands the resulting `(fd, node)` to the call engine, which
//! feeds it into `pipewiresrc`.
//!
//! No GTK/window handle is passed (dialog appears unparented — acceptable
//! for an explicit user action like pressing "share").

use std::os::fd::OwnedFd;

use ashpd::desktop::screencast::{CursorMode, Screencast, SourceType};
use ashpd::desktop::PersistMode;

/// A user-approved PipeWire screen stream.
#[derive(Debug)]
pub struct PipeWireStream {
    /// Remote fd. The engine keeps it alive for the call duration.
    pub fd: OwnedFd,
    /// PipeWire node id for `pipewiresrc path=`.
    pub node: u32,
}

/// Whether screen capture should go through the portal.
///
/// Wayland always needs it; X11 uses `ximagesrc` directly. `YM_SHARE_PORTAL`
/// forces the choice (`1`/`true` = portal, `0`/`false` = ximagesrc).
pub fn portal_recommended() -> bool {
    if let Ok(v) = std::env::var("YM_SHARE_PORTAL") {
        let v = v.to_lowercase();
        if v == "1" || v == "true" {
            return true;
        }
        if v == "0" || v == "false" {
            return false;
        }
    }
    if let Ok(t) = std::env::var("XDG_SESSION_TYPE") {
        return t.eq_ignore_ascii_case("wayland");
    }
    std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("DISPLAY").is_none()
}

/// Run the portal picker: session → sources → user dialog → stream + fd.
///
/// Shows a system dialog; `Err` on cancel/timeout/portal errors.
pub async fn pick_screen() -> Result<PipeWireStream, String> {
    let proxy = Screencast::new()
        .await
        .map_err(|e| format!("portal screencast: {e}"))?;
    let session = proxy
        .create_session()
        .await
        .map_err(|e| format!("portal session: {e}"))?;
    proxy
        .select_sources(
            &session,
            CursorMode::Embedded,
            SourceType::Monitor | SourceType::Window,
            false,
            None,
            PersistMode::DoNot,
        )
        .await
        .map_err(|e| format!("portal select: {e}"))?;
    let streams = proxy
        .start(&session, None)
        .await
        .map_err(|e| format!("portal start (dialog?): {e}"))?
        .response()
        .map_err(|e| format!("portal response: {e}"))?;
    let stream = streams
        .streams()
        .first()
        .ok_or_else(|| "no stream selected (dialog cancelled?)".to_string())?;
    let node = stream.pipe_wire_node_id();
    let fd = proxy
        .open_pipe_wire_remote(&session)
        .await
        .map_err(|e| format!("pipewire remote: {e}"))?;
    Ok(PipeWireStream { fd, node })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portal_recommendation_matrix() {
        // Env override wins over session detection.
        std::env::set_var("YM_SHARE_PORTAL", "1");
        assert!(portal_recommended());
        std::env::set_var("YM_SHARE_PORTAL", "0");
        assert!(!portal_recommended());
        std::env::remove_var("YM_SHARE_PORTAL");
        // No-panic smoke on real env (result depends on host).
        let _ = portal_recommended();
    }

    /// Portal session smoke: needs a user bus + xdg-desktop-portal.
    /// Skips (green) where unavailable — the interactive `Start` dialog is
    /// covered by the manual checklist, not CI.
    #[tokio::test]
    async fn portal_session_smoke() {
        let proxy = match Screencast::new().await {
            Ok(p) => p,
            Err(e) => {
                eprintln!("SKIP portal test (no proxy): {e}");
                return;
            }
        };
        if let Err(e) = proxy.create_session().await {
            eprintln!("SKIP portal test (no session): {e}");
        }
    }
}

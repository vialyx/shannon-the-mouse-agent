use crate::buffer::MouseSample;
use anyhow::{bail, Result};
use crossbeam_channel::Sender;
use rdev::{listen, Event, EventType};
use std::time::{SystemTime, UNIX_EPOCH};

/// Start listening for OS mouse-move events and forward each sample through
/// `sender`.  This function **blocks indefinitely** and should be called from
/// a dedicated thread (e.g. `tokio::task::spawn_blocking`).
///
/// # Platform notes
/// * **macOS** – uses `CGEventTap`; the process must have the *Accessibility*
///   permission granted in *System Settings → Privacy & Security →
///   Accessibility*.  A diagnostic message is printed if the hook fails.
/// * **Windows** – uses `SetWindowsHookEx`; no elevated privileges required.
/// * **Linux** – uses X11/XInput2.  If no display server is available the
///   function returns an error with a helpful message.
pub fn start_capture(sender: Sender<MouseSample>) -> Result<()> {
    let result = listen(move |event: Event| {
        if let EventType::MouseMove { x, y } = event.event_type {
            let timestamp_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64;

            // Ignore send errors – they simply mean the receiver was dropped
            // (agent is shutting down).
            let _ = sender.send(MouseSample { x, y, timestamp_ms });
        }
    });

    if let Err(e) = result {
        print_platform_hint();
        bail!("Mouse capture failed: {:?}", e);
    }

    Ok(())
}

fn print_platform_hint() {
    #[cfg(target_os = "macos")]
    {
        eprintln!("⚠  Mouse capture failed on macOS.");
        eprintln!("   Please grant Accessibility permission to this application:");
        eprintln!(
            "   System Settings → Privacy & Security → Accessibility → enable your terminal."
        );
    }

    #[cfg(target_os = "linux")]
    {
        eprintln!("⚠  Mouse capture failed on Linux.");
        eprintln!("   Ensure a running X11 display is available ($DISPLAY is set).");
        eprintln!("   On Wayland, XWayland must be active or rdev must find libinput.");
    }

    #[cfg(target_os = "windows")]
    {
        eprintln!("⚠  Mouse capture failed on Windows.");
        eprintln!("   SetWindowsHookEx requires a running message loop in the same thread.");
    }
}

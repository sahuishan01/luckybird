use tokio::sync::mpsc;
use tracing::{debug, warn};

use lb_tui::{read_event, EditorEvent};

use crate::app::App;

// ─── Event-reader thread ─────────────────────────────────────────────────────

/// Spawn a dedicated OS thread that blocks on `crossterm::event::read()` and
/// forwards events into a tokio channel.
///
/// The thread runs for the lifetime of the process; dropping the channel
/// receiver will cause the next send to fail and the thread will exit.
pub fn spawn_event_thread() -> mpsc::UnboundedReceiver<EditorEvent> {
    let (tx, rx) = mpsc::unbounded_channel::<EditorEvent>();
    std::thread::spawn(move || {
        debug!("TUI event thread started");
        loop {
            match read_event() {
                Ok(ev) => {
                    if tx.send(ev).is_err() {
                        // Receiver dropped — app is shutting down.
                        break;
                    }
                }
                Err(e) => {
                    warn!("TUI event read error: {e}");
                    break;
                }
            }
        }
        debug!("TUI event thread exited");
    });
    rx
}

// ─── Main event loop ─────────────────────────────────────────────────────────

/// Run the editor event loop until the user quits.
///
/// * `app`      — fully initialised app (with open buffers, TUI entered).
/// * `event_rx` — receiver from `spawn_event_thread`.
///
/// Bursts of events (paste, key repeat, rapid resize) are coalesced into a
/// single render: we await the next event, then drain everything else queued
/// behind it before redrawing. This keeps redraw count proportional to user-
/// perceivable change instead of raw event count.
pub async fn run(
    app: &mut App,
    event_rx: &mut mpsc::UnboundedReceiver<EditorEvent>,
    job_rx: &mut mpsc::UnboundedReceiver<String>,
) {
    if let Err(e) = app.render() {
        warn!("initial render error: {e}");
    }

    loop {
        // Block for at least one event.
        let mut ev = tokio::select! {
            ev = event_rx.recv() => match ev {
                Some(ev) => ev,
                None => {
                    debug!("event channel closed; exiting loop");
                    break;
                }
            },
            msg = job_rx.recv() => {
                if let Some(msg) = msg {
                    app.status_msg = Some(msg);
                    if let Err(e) = app.render() {
                        warn!("render error: {e}");
                    }
                }
                continue;
            }
        };

        // Process this event plus any siblings already queued behind it.
        let mut should_quit = false;
        loop {
            match app.handle_event(ev) {
                Ok(true) => {
                    should_quit = true;
                    break;
                }
                Ok(false) => {}
                Err(e) => {
                    app.status_msg = Some(format!("Error: {e}"));
                    warn!("event handler error: {e}");
                }
            }
            match event_rx.try_recv() {
                Ok(next) => ev = next,
                Err(_) => break,
            }
        }

        app.poll_lsp_events();

        if let Err(e) = app.render() {
            warn!("render error: {e}");
        }

        if should_quit || app.quit {
            break;
        }
    }
}

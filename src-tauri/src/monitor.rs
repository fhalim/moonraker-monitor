//! Background poller: emits status to the UI, raises desktop notifications on
//! transitions. Runs in the backend so it keeps working while the window is hidden.

use crate::{moonraker, tray, AppState};
use serde::Serialize;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// Consecutive failures before the printer counts as unreachable.
const UNREACHABLE_AFTER: u32 = 3;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPayload {
    pub reachable: bool,
    pub status: Option<moonraker::PrinterStatus>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snapshot {
    pub reachable: bool,
    pub print_state: String,
    pub klippy: String,
    pub filename: String,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    Complete(String),
    Paused(String),
    PrintError(String),
    KlippyDown(String),
    Unreachable,
}

/// The first sample (prev == None) is a silent baseline. Events fire only on
/// transitions *into* a state, never while staying in it.
pub fn detect_events(prev: Option<&Snapshot>, next: &Snapshot) -> Vec<Event> {
    let Some(prev) = prev else { return vec![] };
    let mut events = vec![];
    if prev.reachable && !next.reachable {
        events.push(Event::Unreachable);
    }
    if prev.print_state != next.print_state {
        match next.print_state.as_str() {
            "complete" => events.push(Event::Complete(next.filename.clone())),
            "paused" => events.push(Event::Paused(next.filename.clone())),
            "error" => events.push(Event::PrintError(if next.message.is_empty() {
                next.filename.clone()
            } else {
                next.message.clone()
            })),
            _ => {}
        }
    }
    if prev.klippy != next.klippy && matches!(next.klippy.as_str(), "shutdown" | "error") {
        events.push(Event::KlippyDown(next.klippy.clone()));
    }
    events
}

fn describe(event: &Event) -> (&'static str, String) {
    match event {
        Event::Complete(f) => ("Print complete", f.clone()),
        Event::Paused(f) => ("Print paused", f.clone()),
        Event::PrintError(m) => ("Print error", m.clone()),
        Event::KlippyDown(s) => ("Printer firmware error", format!("Klipper state: {s}")),
        Event::Unreachable => ("Printer unreachable", "Moonraker is not responding".into()),
    }
}

fn notify(app: &AppHandle, events: &[Event]) {
    for event in events {
        let (title, body) = describe(event);
        let _ = app.notification().builder().title(title).body(body).show();
    }
}

fn tooltip(p: &StatusPayload) -> String {
    match (&p.status, p.reachable) {
        (Some(s), true) if s.state == "printing" || s.state == "paused" => {
            format!("{} {:.0}% - {}", s.state, s.progress * 100.0, s.filename)
        }
        (Some(s), true) => format!("Moonraker: {} (klippy {})", s.state, s.klippy_state),
        _ => "Moonraker: unreachable".into(),
    }
}

fn publish(app: &AppHandle, payload: StatusPayload) {
    tray::set_tooltip(app, &tooltip(&payload));
    let _ = app.emit("printer-status", &payload);
    *app.state::<AppState>().last.lock().unwrap() = Some(payload);
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_url: Option<String> = None;
        let mut prev: Option<Snapshot> = None;
        let mut fails = 0u32;
        loop {
            let state = app.state::<AppState>();
            // Read every tick so a saved URL repoints the poller and resets the baseline.
            let url = state.url.lock().unwrap().clone();
            if url != last_url {
                last_url = url.clone();
                prev = None;
                fails = 0;
            }
            if let Some(url) = url {
                match moonraker::query_status(&state.client, &url).await {
                    Ok(status) => {
                        fails = 0;
                        let snap = Snapshot {
                            reachable: true,
                            print_state: status.state.clone(),
                            klippy: status.klippy_state.clone(),
                            filename: status.filename.clone(),
                            message: status.message.clone(),
                        };
                        notify(&app, &detect_events(prev.as_ref(), &snap));
                        prev = Some(snap);
                        publish(&app, StatusPayload { reachable: true, status: Some(status), error: None });
                    }
                    Err(err) => {
                        fails += 1;
                        if fails >= UNREACHABLE_AFTER {
                            if let Some(p) = prev.as_mut() {
                                let mut down = p.clone();
                                down.reachable = false;
                                notify(&app, &detect_events(Some(p), &down));
                                *p = down;
                            }
                            publish(&app, StatusPayload { reachable: false, status: None, error: Some(err) });
                        }
                    }
                }
            }
            tokio::time::sleep(POLL_INTERVAL).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snap(print: &str, klippy: &str) -> Snapshot {
        Snapshot {
            reachable: true,
            print_state: print.into(),
            klippy: klippy.into(),
            filename: "a.gcode".into(),
            message: String::new(),
        }
    }

    #[test]
    fn baseline_is_silent() {
        assert!(detect_events(None, &snap("complete", "ready")).is_empty());
        assert!(detect_events(None, &snap("error", "shutdown")).is_empty());
    }

    #[test]
    fn transitions_into_states() {
        let printing = snap("printing", "ready");
        assert_eq!(detect_events(Some(&printing), &snap("complete", "ready")), vec![Event::Complete("a.gcode".into())]);
        assert_eq!(detect_events(Some(&printing), &snap("paused", "ready")), vec![Event::Paused("a.gcode".into())]);
        assert_eq!(detect_events(Some(&printing), &snap("error", "ready")), vec![Event::PrintError("a.gcode".into())]);
        assert_eq!(detect_events(Some(&snap("standby", "ready")), &snap("standby", "shutdown")), vec![Event::KlippyDown("shutdown".into())]);
    }

    #[test]
    fn no_repeat_while_staying() {
        let complete = snap("complete", "ready");
        assert!(detect_events(Some(&complete), &complete).is_empty());
        let down = snap("standby", "shutdown");
        assert!(detect_events(Some(&down), &down).is_empty());
    }

    #[test]
    fn other_transitions_silent() {
        assert!(detect_events(Some(&snap("paused", "ready")), &snap("printing", "ready")).is_empty());
        assert!(detect_events(Some(&snap("printing", "ready")), &snap("cancelled", "ready")).is_empty());
    }

    #[test]
    fn unreachable_fires_once() {
        let up = snap("printing", "ready");
        let mut down = up.clone();
        down.reachable = false;
        assert_eq!(detect_events(Some(&up), &down), vec![Event::Unreachable]);
        assert!(detect_events(Some(&down), &down).is_empty());
    }
}

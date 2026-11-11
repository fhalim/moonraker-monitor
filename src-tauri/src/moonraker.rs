//! Moonraker HTTP client and status parsing.
//! Parsing rules ported from esp32-moonraker/src/moonraker.py.

use reqwest::{Client, Url};
use serde::{Deserialize, Serialize};
use serde_json::Value;

const OBJECTS_QUERY: &str = "/printer/objects/query\
?print_stats=state,filename,print_duration,message\
&display_status=progress\
&virtual_sdcard=progress\
&extruder=temperature,target\
&heater_bed=temperature,target\
&webhooks=state,state_message";

/// Below this, progress is noise: report no ETA.
const MIN_PROGRESS_FOR_ETA: f64 = 0.01;

pub fn normalize_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim();
    let url = Url::parse(trimmed).map_err(|e| format!("Invalid URL: {e}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err("URL must start with http:// or https://".into());
    }
    if url.host_str().is_none() {
        return Err("URL has no host".into());
    }
    Ok(trimmed.trim_end_matches('/').to_string())
}

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PrinterStatus {
    /// print_stats.state: standby/printing/paused/complete/cancelled/error, or "unknown"
    pub state: String,
    /// webhooks.state / server klippy_state: ready/startup/shutdown/error/disconnected
    pub klippy_state: String,
    pub message: String,
    pub filename: String,
    pub progress: f64,
    pub print_duration: f64,
    pub eta_seconds: Option<f64>,
    pub extruder_temp: f64,
    pub extruder_target: f64,
    pub bed_temp: f64,
    pub bed_target: f64,
}

fn num(v: &Value, obj: &str, key: &str) -> f64 {
    v[obj][key].as_f64().unwrap_or(0.0)
}

fn text(v: &Value, obj: &str, key: &str) -> String {
    v[obj][key].as_str().unwrap_or("").to_string()
}

/// `status` is the `result.status` object of /printer/objects/query.
pub fn parse_status(status: &Value) -> PrinterStatus {
    let display = num(status, "display_status", "progress");
    let sdcard = num(status, "virtual_sdcard", "progress");
    // display_status.progress is slicer-reported (M73); prefer it when set.
    let progress = if display > 0.0 { display } else { sdcard };
    let print_duration = num(status, "print_stats", "print_duration");
    let eta_seconds = (progress > MIN_PROGRESS_FOR_ETA)
        .then(|| print_duration / progress - print_duration);
    let state = text(status, "print_stats", "state");
    let klippy = text(status, "webhooks", "state");
    PrinterStatus {
        state: if state.is_empty() { "standby".into() } else { state },
        klippy_state: if klippy.is_empty() { "ready".into() } else { klippy },
        message: text(status, "print_stats", "message"),
        filename: text(status, "print_stats", "filename"),
        progress,
        print_duration,
        eta_seconds,
        extruder_temp: num(status, "extruder", "temperature"),
        extruder_target: num(status, "extruder", "target"),
        bed_temp: num(status, "heater_bed", "temperature"),
        bed_target: num(status, "heater_bed", "target"),
    }
}

async fn get_json(client: &Client, url: &str) -> Result<Value, String> {
    let resp = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Cannot reach {url}: {e}"))?;
    let status = resp.status();
    if !status.is_success() {
        return Err(format!("{url} returned HTTP {status}"));
    }
    resp.json::<Value>()
        .await
        .map_err(|e| format!("{url} did not return JSON: {e}"))
}

/// GET /server/info; requires a Moonraker-shaped `result` object.
pub async fn server_info(client: &Client, base: &str) -> Result<Value, String> {
    let body = get_json(client, &format!("{base}/server/info")).await?;
    match body.get("result") {
        Some(r) if r.is_object() => Ok(r.clone()),
        _ => Err("Server answered but this does not look like Moonraker (no `result` in /server/info)".into()),
    }
}

/// Full status. If klippy is down the objects query fails; fall back to
/// /server/info so a disconnected klippy is not reported as unreachable.
pub async fn query_status(client: &Client, base: &str) -> Result<PrinterStatus, String> {
    match get_json(client, &format!("{base}{OBJECTS_QUERY}")).await {
        Ok(body) => Ok(parse_status(&body["result"]["status"])),
        Err(query_err) => {
            let info = server_info(client, base).await.map_err(|_| query_err)?;
            let klippy = info["klippy_state"].as_str().unwrap_or("disconnected");
            Ok(PrinterStatus {
                state: "unknown".into(),
                klippy_state: klippy.into(),
                message: String::new(),
                filename: String::new(),
                progress: 0.0,
                print_duration: 0.0,
                eta_seconds: None,
                extruder_temp: 0.0,
                extruder_target: 0.0,
                bed_temp: 0.0,
                bed_target: 0.0,
            })
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Webcam {
    #[serde(default)]
    pub uid: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub service: String,
    #[serde(default, alias = "stream_url")]
    pub stream_url: String,
    #[serde(default, alias = "flip_horizontal")]
    pub flip_horizontal: bool,
    #[serde(default, alias = "flip_vertical")]
    pub flip_vertical: bool,
    #[serde(default, alias = "aspect_ratio")]
    pub aspect_ratio: String,
}

/// Absolute URLs pass through. A leading-`/` URL is served by the web proxy
/// (nginx) on the host's default port, not Moonraker's, so drop the port.
pub fn resolve_url(base: &str, url: &str) -> String {
    if url.starts_with('/') {
        if let Ok(mut b) = Url::parse(base) {
            let _ = b.set_port(None);
            b.set_path("");
            return format!("{}{url}", b.as_str().trim_end_matches('/'));
        }
    }
    url.to_string()
}

/// GET /server/webcams/list with stream URLs resolved against `base`.
pub async fn list_webcams(client: &Client, base: &str) -> Result<Vec<Webcam>, String> {
    let body = get_json(client, &format!("{base}/server/webcams/list")).await?;
    parse_webcams(base, &body)
}

fn parse_webcams(base: &str, body: &Value) -> Result<Vec<Webcam>, String> {
    let list = body["result"]["webcams"]
        .as_array()
        .ok_or("No webcam list in response")?;
    Ok(list
        .iter()
        .filter_map(|v| serde_json::from_value::<Webcam>(v.clone()).ok())
        .filter(|w| !w.uid.is_empty())
        .map(|mut w| {
            w.stream_url = resolve_url(base, &w.stream_url);
            w
        })
        .collect())
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Pause,
    Resume,
    Cancel,
    EmergencyStop,
    FirmwareRestart,
    Home,
}

impl Action {
    fn path(self) -> &'static str {
        match self {
            Action::Pause => "/printer/print/pause",
            Action::Resume => "/printer/print/resume",
            Action::Cancel => "/printer/print/cancel",
            Action::EmergencyStop => "/printer/emergency_stop",
            Action::FirmwareRestart => "/printer/firmware_restart",
            Action::Home => "/printer/gcode/script?script=G28",
        }
    }
}

pub async fn post_action(client: &Client, base: &str, action: Action) -> Result<(), String> {
    let url = format!("{base}{}", action.path());
    let resp = client
        .post(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Printer returned HTTP {}", resp.status()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_accepts_and_trims() {
        assert_eq!(normalize_url(" http://printer.local:7125/ ").unwrap(), "http://printer.local:7125");
        assert_eq!(normalize_url("https://p.example").unwrap(), "https://p.example");
        assert_eq!(normalize_url("http://10.0.0.5").unwrap(), "http://10.0.0.5");
    }

    #[test]
    fn normalize_rejects_bad_input() {
        for bad in ["", "not a url", "localhost:7125", "ftp://host", "printer.local:7125", "http://"] {
            assert!(normalize_url(bad).is_err(), "should reject {bad:?}");
        }
    }

    const LIVE: &str = r#"{"result":{"eventtime":9901.09,"status":{
      "print_stats":{"filename":"air_deflector_PLA_2h57m.gcode","print_duration":6262.37,"state":"printing","message":""},
      "virtual_sdcard":{"progress":0.467},
      "extruder":{"temperature":219.53,"target":220.0},
      "heater_bed":{"temperature":49.63,"target":50.0},
      "webhooks":{"state":"ready","state_message":"Printer is ready"},
      "display_status":{"progress":0.61,"message":null}}}}"#;

    #[test]
    fn parses_live_fixture() {
        let v: Value = serde_json::from_str(LIVE).unwrap();
        let s = parse_status(&v["result"]["status"]);
        assert_eq!(s.state, "printing");
        assert_eq!(s.klippy_state, "ready");
        assert_eq!(s.progress, 0.61); // display_status preferred
        assert_eq!(s.filename, "air_deflector_PLA_2h57m.gcode");
        let eta = s.eta_seconds.unwrap();
        assert!((eta - (6262.37 / 0.61 - 6262.37)).abs() < 1e-6);
        assert_eq!(s.bed_target, 50.0);
    }

    #[test]
    fn progress_falls_back_to_sdcard_and_no_eta_when_tiny() {
        let v: Value = serde_json::from_str(
            r#"{"display_status":{"progress":0.0},"virtual_sdcard":{"progress":0.005},"print_stats":{"state":"printing"}}"#,
        )
        .unwrap();
        let s = parse_status(&v);
        assert_eq!(s.progress, 0.005);
        assert!(s.eta_seconds.is_none());
    }

    const WEBCAMS: &str = r#"{"result":{"webcams":[
      {"name":"Example","enabled":false,"aspect_ratio":"4:3","service":"mjpegstreamer","stream_url":"http://your_IP:8080/?action=stream","flip_horizontal":false,"flip_vertical":false,"uid":"fea116be"},
      {"name":"cam","enabled":true,"aspect_ratio":"4:3","service":"mjpegstreamer","stream_url":"/webcam/?action=stream","flip_horizontal":true,"flip_vertical":false,"uid":"8e5002de"}]}}"#;

    #[test]
    fn resolve_url_cases() {
        let base = "http://printer.local:7125";
        assert_eq!(resolve_url(base, "/webcam/?action=stream"), "http://printer.local/webcam/?action=stream");
        assert_eq!(resolve_url(base, "http://x:8080/?a=1"), "http://x:8080/?a=1");
        assert_eq!(resolve_url("https://p.local:7125", "/w"), "https://p.local/w");
    }

    #[test]
    fn parses_webcam_fixture() {
        let v: Value = serde_json::from_str(WEBCAMS).unwrap();
        let w = parse_webcams("http://printer.local:7125", &v).unwrap();
        assert_eq!(w.len(), 2);
        assert!(!w[0].enabled);
        assert_eq!(w[0].stream_url, "http://your_IP:8080/?action=stream");
        assert_eq!(w[1].uid, "8e5002de");
        assert_eq!(w[1].stream_url, "http://printer.local/webcam/?action=stream");
        assert!(w[1].flip_horizontal);
        assert_eq!(w[1].aspect_ratio, "4:3");
    }
}

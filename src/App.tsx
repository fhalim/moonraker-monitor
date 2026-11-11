import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Action, AppConfig, Discovery, StatusPayload, Webcam, availableActions, formatDuration } from "./printer";
import "./App.css";

function UrlForm({ initial, savedCameras, onSaved, onCancel }: { initial: string; savedCameras: string[]; onSaved: (cfg: AppConfig) => void; onCancel?: () => void }) {
  const [url, setUrl] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [found, setFound] = useState<Discovery | null>(null);
  const [selected, setSelected] = useState<string[]>([]);

  async function save(baseUrl: string, cameras: string[]) {
    await invoke<string>("save_config", { url: baseUrl, cameras });
    onSaved({ baseUrl, cameras });
  }

  async function connect(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      const d = await invoke<Discovery>("discover", { url });
      if (d.webcams.length === 0) {
        await save(d.baseUrl, []);
        return;
      }
      const known = d.webcams.filter((w) => savedCameras.includes(w.uid));
      const initialSel = known.length > 0 ? known : d.webcams.filter((w) => w.enabled);
      setSelected(initialSel.map((w) => w.uid));
      setFound(d);
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  async function saveSelection() {
    if (!found) return;
    setBusy(true);
    setError(null);
    try {
      await save(found.baseUrl, selected);
    } catch (err) {
      setError(String(err));
      setBusy(false);
    }
  }

  if (found) {
    return (
      <div className="card">
        <label>Cameras to monitor</label>
        {found.webcams.map((w) => (
          <label key={w.uid} className="check">
            <input
              type="checkbox"
              checked={selected.includes(w.uid)}
              onChange={(e) => setSelected(e.target.checked ? [...selected, w.uid] : selected.filter((u) => u !== w.uid))}
            />
            <span>{w.name}</span>
            <span className="muted">{w.service}{w.enabled ? "" : " · disabled"}</span>
          </label>
        ))}
        {error && <p className="error" role="alert">{error}</p>}
        <div className="row">
          <button onClick={saveSelection} disabled={busy}>{busy ? "Saving…" : "Save"}</button>
          <button type="button" onClick={() => setFound(null)} disabled={busy}>Back</button>
        </div>
      </div>
    );
  }

  return (
    <form onSubmit={connect} className="card">
      <label htmlFor="url">Moonraker base URL</label>
      <input id="url" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="http://printer.local:7125" autoFocus />
      {error && <p className="error" role="alert">{error}</p>}
      <div className="row">
        <button type="submit" disabled={busy || !url.trim()}>{busy ? "Checking…" : "Connect"}</button>
        {onCancel && <button type="button" onClick={onCancel}>Cancel</button>}
      </div>
    </form>
  );
}

function Camera({ cam }: { cam: Webcam }) {
  const [failed, setFailed] = useState(false);
  const [w, h] = cam.aspectRatio.split(":").map(Number);
  const ratio = w > 0 && h > 0 ? `${w} / ${h}` : "4 / 3";
  const flip = `scale(${cam.flipHorizontal ? -1 : 1}, ${cam.flipVertical ? -1 : 1})`;
  const supported = cam.service.startsWith("mjpegstreamer");
  return (
    <div className="card">
      <span className="muted">{cam.name}</span>
      {!supported ? (
        <p className="error">Unsupported service: {cam.service}</p>
      ) : failed ? (
        <p className="error">Stream unavailable</p>
      ) : (
        <img className="cam" src={cam.streamUrl} alt={cam.name} style={{ aspectRatio: ratio, transform: flip }} onError={() => setFailed(true)} />
      )}
    </div>
  );
}

/** Streams stay open while mounted, so render nothing while the window is hidden to tray. */
function Cameras({ baseUrl, selected }: { baseUrl: string; selected: string[] }) {
  const [cams, setCams] = useState<Webcam[]>([]);
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    if (selected.length === 0) {
      setCams([]);
      return;
    }
    invoke<Webcam[]>("get_webcams").then(setCams, () => setCams([]));
  }, [baseUrl, selected]);

  useEffect(() => {
    const unlisten = listen<boolean>("window-visible", (e) => setVisible(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  if (!visible) return null;
  return <>{cams.map((c) => <Camera key={c.uid} cam={c} />)}</>;
}

const CONFIRM: Partial<Record<Action, string>> = {
  cancel: "Cancel the print?",
  emergency_stop: "Emergency stop?",
  firmware_restart: "Restart firmware?",
};

function Controls({ payload }: { payload: StatusPayload | null }) {
  const enabled = availableActions(payload);
  const [confirming, setConfirming] = useState<Action | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function run(action: Action) {
    setConfirming(null);
    setError(null);
    try {
      await invoke("printer_action", { action });
    } catch (err) {
      setError(String(err));
    }
  }

  function click(action: Action) {
    if (CONFIRM[action]) setConfirming(action);
    else run(action);
  }

  const buttons: [Action, string][] = [
    ["pause", "Pause"],
    ["resume", "Resume"],
    ["cancel", "Cancel print"],
    ["home", "Home"],
    ["firmware_restart", "Firmware restart"],
    ["emergency_stop", "E-stop"],
  ];

  return (
    <div className="card">
      <div className="row wrap">
        {buttons.map(([action, label]) => (
          <button key={action} disabled={!enabled[action]} onClick={() => click(action)} className={action === "emergency_stop" ? "danger" : ""}>
            {label}
          </button>
        ))}
      </div>
      {confirming && (
        <div className="row confirm" role="alertdialog">
          <span>{CONFIRM[confirming]}</span>
          <button className="danger" onClick={() => run(confirming)}>Yes</button>
          <button onClick={() => setConfirming(null)}>No</button>
        </div>
      )}
      {error && <p className="error" role="alert">{error}</p>}
    </div>
  );
}

function Dashboard({ payload }: { payload: StatusPayload | null }) {
  const s = payload?.status;
  return (
    <>
      <div className="card">
        <div className="row between">
          <span className={`badge ${payload?.reachable ? "ok" : "bad"}`}>{payload === null ? "connecting…" : payload.reachable ? "connected" : "unreachable"}</span>
          {s && <span className="badge">klippy: {s.klippyState}</span>}
          {s && <span className="badge">{s.state}</span>}
        </div>
        {payload && !payload.reachable && <p className="error">{payload.error}</p>}
        {s && (
          <>
            <p className="filename">{s.filename || "No file"}</p>
            <progress value={s.progress} max={1} />
            <div className="row between">
              <span>{(s.progress * 100).toFixed(0)}%</span>
              <span>elapsed {formatDuration(s.printDuration)}</span>
              <span>remaining {formatDuration(s.etaSeconds)}</span>
            </div>
            <div className="row between temps">
              <span>Nozzle {s.extruderTemp.toFixed(0)}° / {s.extruderTarget.toFixed(0)}°</span>
              <span>Bed {s.bedTemp.toFixed(0)}° / {s.bedTarget.toFixed(0)}°</span>
            </div>
          </>
        )}
      </div>
      <Controls payload={payload} />
    </>
  );
}

export default function App() {
  const [config, setConfig] = useState<AppConfig | undefined>(undefined);
  const [editing, setEditing] = useState(false);
  const [payload, setPayload] = useState<StatusPayload | null>(null);
  const [testMsg, setTestMsg] = useState<string | null>(null);

  useEffect(() => {
    invoke<AppConfig>("get_config").then(setConfig);
    invoke<StatusPayload | null>("get_status").then(setPayload);
    const unlisten = listen<StatusPayload>("printer-status", (e) => setPayload(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  if (config === undefined) return null;
  const url = config.baseUrl;

  if (!url || editing) {
    return (
      <main>
        <h1>Moonraker Monitor</h1>
        <UrlForm
          initial={url ?? ""}
          savedCameras={config.cameras}
          onSaved={(c) => {
            setConfig(c);
            setPayload(null);
            setEditing(false);
          }}
          onCancel={url ? () => setEditing(false) : undefined}
        />
      </main>
    );
  }

  return (
    <main>
      <div className="row between">
        <h1>Moonraker Monitor</h1>
        <div className="row">
          <button
            onClick={() =>
              invoke("test_notification").then(
                () => setTestMsg("Sent"),
                (e) => setTestMsg(String(e)),
              )
            }
          >
            Test notification
          </button>
          <button onClick={() => setEditing(true)}>Settings</button>
        </div>
      </div>
      <p className="muted">{url}{testMsg && ` · ${testMsg}`}</p>
      <Dashboard payload={payload} />
      <Cameras baseUrl={url} selected={config.cameras} />
    </main>
  );
}

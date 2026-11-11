import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Action, StatusPayload, availableActions, formatDuration } from "./printer";
import "./App.css";

function UrlForm({ initial, onSaved, onCancel }: { initial: string; onSaved: (url: string) => void; onCancel?: () => void }) {
  const [url, setUrl] = useState(initial);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    setBusy(true);
    setError(null);
    try {
      onSaved(await invoke<string>("save_config", { url }));
    } catch (err) {
      setError(String(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <form onSubmit={submit} className="card">
      <label htmlFor="url">Moonraker base URL</label>
      <input id="url" value={url} onChange={(e) => setUrl(e.target.value)} placeholder="http://printer.local:7125" autoFocus />
      {error && <p className="error" role="alert">{error}</p>}
      <div className="row">
        <button type="submit" disabled={busy || !url.trim()}>{busy ? "Checking…" : "Validate & Save"}</button>
        {onCancel && <button type="button" onClick={onCancel}>Cancel</button>}
      </div>
    </form>
  );
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
  const [url, setUrl] = useState<string | null | undefined>(undefined);
  const [editing, setEditing] = useState(false);
  const [payload, setPayload] = useState<StatusPayload | null>(null);
  const [testMsg, setTestMsg] = useState<string | null>(null);

  useEffect(() => {
    invoke<string | null>("get_config").then(setUrl);
    invoke<StatusPayload | null>("get_status").then(setPayload);
    const unlisten = listen<StatusPayload>("printer-status", (e) => setPayload(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
  }, []);

  if (url === undefined) return null;

  if (!url || editing) {
    return (
      <main>
        <h1>Moonraker Monitor</h1>
        <UrlForm
          initial={url ?? ""}
          onSaved={(u) => {
            setUrl(u);
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
    </main>
  );
}

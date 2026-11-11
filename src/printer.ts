export interface PrinterStatus {
  state: string;
  klippyState: string;
  message: string;
  filename: string;
  progress: number;
  printDuration: number;
  etaSeconds: number | null;
  extruderTemp: number;
  extruderTarget: number;
  bedTemp: number;
  bedTarget: number;
}

export interface StatusPayload {
  reachable: boolean;
  status: PrinterStatus | null;
  error: string | null;
}

export type Action =
  | "pause"
  | "resume"
  | "cancel"
  | "emergency_stop"
  | "firmware_restart"
  | "home";

/** Which controls are enabled for a given print state. */
export function availableActions(payload: StatusPayload | null): Record<Action, boolean> {
  const none = { pause: false, resume: false, cancel: false, emergency_stop: false, firmware_restart: false, home: false };
  if (!payload?.reachable || !payload.status) return none;
  const { state } = payload.status;
  const printing = state === "printing";
  const paused = state === "paused";
  return {
    pause: printing,
    resume: paused,
    cancel: printing || paused,
    home: !printing && !paused && state !== "unknown",
    firmware_restart: true,
    emergency_stop: true,
  };
}

export function formatDuration(seconds: number | null): string {
  if (seconds === null || seconds < 0) return "--:--";
  const minutes = Math.floor(seconds / 60);
  return `${String(Math.floor(minutes / 60)).padStart(2, "0")}:${String(minutes % 60).padStart(2, "0")}`;
}

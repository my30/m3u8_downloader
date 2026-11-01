import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type JobStatus = "Pending" | "Running" | "Done" | "Error" | "Canceled";
export type OutputFormat = "mp4" | "mkv" | "ts";

/** Must match MAX_LOG_LINES in src-tauri/src/download.rs */
export const MAX_LOG_LINES = 500;

export interface DownloadJob {
  id: string;
  url: string;
  output_path: string;
  filename: string;
  format: OutputFormat;
  start_time: string | null;
  end_time: string | null;
  user_agent: string | null;
  referer: string | null;
  headers: string[];
  status: JobStatus;
  /** Output time reached so far, from ffmpeg `-progress` */
  progress_secs: number | null;
  /** Expected output duration, when known (trim range or VOD duration) */
  total_secs: number | null;
  error: string | null;
  log_lines: string[];
}

export interface StartDownloadArgs {
  url: string;
  output_dir: string;
  filename: string;
  format: OutputFormat;
  start_time: string | null;
  end_time: string | null;
  user_agent: string | null;
  referer: string | null;
  headers: string[];
}

export interface ProgressEvent {
  id: string;
  progress_secs: number;
  total_secs: number | null;
}

export interface LogEvent {
  id: string;
  line: string;
}

export interface StatusEvent {
  id: string;
  status: JobStatus;
  error: string | null;
}

// ── Commands ───────────────────────────────────────────────────────────────

export function startDownload(args: StartDownloadArgs): Promise<string> {
  return invoke<string>("start_download", { args });
}

export function cancelDownload(id: string): Promise<void> {
  return invoke<void>("cancel_download", { id });
}

export function getQueue(): Promise<DownloadJob[]> {
  return invoke<DownloadJob[]>("get_queue");
}

export function clearCompleted(): Promise<void> {
  return invoke<void>("clear_completed");
}

// ── Events ─────────────────────────────────────────────────────────────────

export function onProgress(handler: (e: ProgressEvent) => void): Promise<UnlistenFn> {
  return listen<ProgressEvent>("download_progress", (e) => handler(e.payload));
}

export function onLog(handler: (e: LogEvent) => void): Promise<UnlistenFn> {
  return listen<LogEvent>("download_log", (e) => handler(e.payload));
}

export function onStatusChange(handler: (e: StatusEvent) => void): Promise<UnlistenFn> {
  return listen<StatusEvent>("download_status", (e) => handler(e.payload));
}

// ── Helpers ────────────────────────────────────────────────────────────────

/** Parse HH:MM:SS[.f], MM:SS[.f] or seconds. Mirrors parse_timestamp in Rust. */
export function parseTimestamp(val: string): number | null {
  const parts = val.trim().split(":");
  if (parts.length === 0 || parts.length > 3 || parts.some((p) => p === "")) return null;
  let total = 0;
  for (let i = 0; i < parts.length; i++) {
    const last = i === parts.length - 1;
    if (!(last ? /^\d+(\.\d+)?$/ : /^\d+$/).test(parts[i])) return null;
    const v = Number(parts[i]);
    if (i > 0 && v >= 60) return null;
    total = total * 60 + v;
  }
  return total;
}

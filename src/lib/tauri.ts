import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type JobStatus = "Pending" | "Running" | "Done" | "Error" | "Canceled";
export type OutputFormat = "mp4" | "mkv" | "ts";

export interface DownloadJob {
  id: string;
  url: string;
  output_dir: string;
  filename: string;
  format: OutputFormat;
  start_time: string | null;
  end_time: string | null;
  status: JobStatus;
  progress_secs: number | null; // parsed from ffmpeg time= field
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
}

export interface ProgressEvent {
  id: string;
  progress_secs: number;
  log_line: string;
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

export function onProgress(
  handler: (e: ProgressEvent) => void
): Promise<UnlistenFn> {
  return listen<ProgressEvent>("download_progress", (e) => handler(e.payload));
}

export function onStatusChange(
  handler: (e: StatusEvent) => void
): Promise<UnlistenFn> {
  return listen<StatusEvent>("download_status", (e) => handler(e.payload));
}

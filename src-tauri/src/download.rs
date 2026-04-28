use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
pub enum JobStatus {
    Pending,
    Running,
    Done,
    Error,
    Canceled,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadJob {
    pub id: String,
    pub url: String,
    pub output_dir: String,
    pub filename: String,
    pub format: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub status: JobStatus,
    pub progress_secs: Option<f64>,
    pub error: Option<String>,
    pub log_lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StartDownloadArgs {
    pub url: String,
    pub output_dir: String,
    pub filename: String,
    pub format: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub id: String,
    pub progress_secs: f64,
    pub log_line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusEvent {
    pub id: String,
    pub status: JobStatus,
    pub error: Option<String>,
}

// ── Queue state ────────────────────────────────────────────────────────────

pub struct DownloadManager {
    pub jobs: Mutex<Vec<DownloadJob>>,
    /// Map of job_id -> child process handle (for cancellation)
    pub children: Mutex<HashMap<String, Child>>,
}

impl Default for DownloadManager {
    fn default() -> Self {
        Self {
            jobs: Mutex::new(Vec::new()),
            children: Mutex::new(HashMap::new()),
        }
    }
}

// ── ffmpeg path (sidecar) ──────────────────────────────────────────────────

/// Resolve the bundled ffmpeg sidecar path.
/// Tauri places sidecars next to the executable during dev and inside the bundle on release.
fn ffmpeg_path(app: &AppHandle) -> std::path::PathBuf {
    // During `tauri dev` the sidecar lives in src-tauri/binaries/<target-triple>/ffmpeg[.exe]
    // The tauri-plugin-shell approach is preferred for release, but for simplicity we
    // resolve via the resource directory and fall back to system `ffmpeg`.
    if let Ok(res) = app.path().resource_dir() {
        let candidate = res.join("ffmpeg");
        if candidate.exists() {
            return candidate;
        }
        #[cfg(target_os = "windows")]
        {
            let candidate_exe = res.join("ffmpeg.exe");
            if candidate_exe.exists() {
                return candidate_exe;
            }
        }
    }
    // Fallback: expect ffmpeg in PATH (useful during development)
    std::path::PathBuf::from("ffmpeg")
}

// ── Build ffmpeg argument list ─────────────────────────────────────────────

pub fn build_ffmpeg_args(job: &DownloadJob) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();

    // -ss before -i for fast input seek
    if let Some(ref st) = job.start_time {
        args.extend(["-ss".to_string(), st.clone()]);
    }

    args.extend(["-i".to_string(), job.url.clone()]);

    // -to is an output option (placed after -i)
    if let Some(ref et) = job.end_time {
        args.extend(["-to".to_string(), et.clone()]);
    }

    args.extend([
        "-c".to_string(),
        "copy".to_string(),
        "-bsf:a".to_string(),
        "aac_adtstoasc".to_string(),
        // Overwrite without prompting
        "-y".to_string(),
    ]);

    // Output path
    let sep = if cfg!(windows) { "\\" } else { "/" };
    let output = format!(
        "{}{}{}.{}",
        job.output_dir, sep, job.filename, job.format
    );
    args.push(output);

    args
}

// ── Parse ffmpeg progress from stderr ─────────────────────────────────────

/// Extracts seconds from ffmpeg's `time=HH:MM:SS.ms` stderr output.
pub fn parse_time(line: &str) -> Option<f64> {
    let re = Regex::new(r"time=(\d+):(\d+):(\d+\.\d+)").unwrap();
    let caps = re.captures(line)?;
    let h: f64 = caps[1].parse().ok()?;
    let m: f64 = caps[2].parse().ok()?;
    let s: f64 = caps[3].parse().ok()?;
    Some(h * 3600.0 + m * 60.0 + s)
}

// ── Commands ───────────────────────────────────────────────────────────────

#[tauri::command]
pub fn get_queue(manager: tauri::State<Arc<DownloadManager>>) -> Vec<DownloadJob> {
    manager.jobs.lock().unwrap().clone()
}

#[tauri::command]
pub fn clear_completed(manager: tauri::State<Arc<DownloadManager>>) {
    let mut jobs = manager.jobs.lock().unwrap();
    jobs.retain(|j| j.status == JobStatus::Running || j.status == JobStatus::Pending);
}

#[tauri::command]
pub fn cancel_download(
    id: String,
    manager: tauri::State<Arc<DownloadManager>>,
) {
    // Kill the child process if still running
    {
        let mut children = manager.children.lock().unwrap();
        if let Some(mut child) = children.remove(&id) {
            let _ = child.kill();
        }
    }
    // Update job status
    {
        let mut jobs = manager.jobs.lock().unwrap();
        if let Some(job) = jobs.iter_mut().find(|j| j.id == id) {
            if job.status == JobStatus::Running || job.status == JobStatus::Pending {
                job.status = JobStatus::Canceled;
            }
        }
    }
}

#[tauri::command]
pub fn start_download(
    args: StartDownloadArgs,
    manager: tauri::State<Arc<DownloadManager>>,
    app: AppHandle,
) -> Result<String, String> {
    let id = Uuid::new_v4().to_string();

    let job = DownloadJob {
        id: id.clone(),
        url: args.url,
        output_dir: args.output_dir,
        filename: args.filename,
        format: args.format,
        start_time: args.start_time,
        end_time: args.end_time,
        status: JobStatus::Pending,
        progress_secs: None,
        error: None,
        log_lines: Vec::new(),
    };

    manager.jobs.lock().unwrap().push(job.clone());

    let manager_arc = manager.inner().clone();
    let job_id = id.clone();
    let ffmpeg = ffmpeg_path(&app);

    // Spawn a thread to run ffmpeg and emit events
    std::thread::spawn(move || {
        run_ffmpeg(job_id, job, ffmpeg, manager_arc, app);
    });

    Ok(id)
}

// ── ffmpeg runner (runs in a background thread) ────────────────────────────

fn run_ffmpeg(
    job_id: String,
    job: DownloadJob,
    ffmpeg: std::path::PathBuf,
    manager: Arc<DownloadManager>,
    app: AppHandle,
) {
    let ffmpeg_args = build_ffmpeg_args(&job);

    // Mark running
    {
        let mut jobs = manager.jobs.lock().unwrap();
        if let Some(j) = jobs.iter_mut().find(|j| j.id == job_id) {
            j.status = JobStatus::Running;
        }
    }
    emit_status(&app, &job_id, JobStatus::Running, None);

    let child_result = Command::new(&ffmpeg)
        .args(&ffmpeg_args)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn();

    let mut child = match child_result {
        Ok(c) => c,
        Err(e) => {
            let msg = format!("Failed to start ffmpeg: {e}");
            mark_error(&manager, &job_id, &msg);
            emit_status(&app, &job_id, JobStatus::Error, Some(msg));
            return;
        }
    };

    let stderr = child.stderr.take().expect("stderr was piped");

    // Store child handle for cancellation
    {
        manager.children.lock().unwrap().insert(job_id.clone(), child);
    }

    // Read stderr line by line
    let reader = BufReader::new(stderr);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };

        // Check if job was canceled
        {
            let jobs = manager.jobs.lock().unwrap();
            if let Some(j) = jobs.iter().find(|j| j.id == job_id) {
                if j.status == JobStatus::Canceled {
                    return;
                }
            }
        }

        let progress_secs = parse_time(&line);

        // Append log line to job
        {
            let mut jobs = manager.jobs.lock().unwrap();
            if let Some(j) = jobs.iter_mut().find(|j| j.id == job_id) {
                j.log_lines.push(line.clone());
                if let Some(secs) = progress_secs {
                    j.progress_secs = Some(secs);
                }
            }
        }

        // Emit progress event
        if let Some(secs) = progress_secs {
            let _ = app.emit(
                "download_progress",
                ProgressEvent {
                    id: job_id.clone(),
                    progress_secs: secs,
                    log_line: line,
                },
            );
        }
    }

    // Retrieve child and wait for exit
    let exit_status = {
        let mut children = manager.children.lock().unwrap();
        children
            .remove(&job_id)
            .and_then(|mut c| c.wait().ok())
    };

    // Final status
    {
        let mut jobs = manager.jobs.lock().unwrap();
        if let Some(j) = jobs.iter_mut().find(|j| j.id == job_id) {
            // Don't overwrite Canceled
            if j.status != JobStatus::Canceled {
                let success = exit_status.map(|s| s.success()).unwrap_or(false);
                if success {
                    j.status = JobStatus::Done;
                    emit_status(&app, &job_id, JobStatus::Done, None);
                } else {
                    let msg = "ffmpeg exited with a non-zero status code.".to_string();
                    j.status = JobStatus::Error;
                    j.error = Some(msg.clone());
                    emit_status(&app, &job_id, JobStatus::Error, Some(msg));
                }
            }
        }
    }
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn mark_error(manager: &Arc<DownloadManager>, id: &str, msg: &str) {
    let mut jobs = manager.jobs.lock().unwrap();
    if let Some(j) = jobs.iter_mut().find(|j| j.id == id) {
        j.status = JobStatus::Error;
        j.error = Some(msg.to_string());
    }
}

fn emit_status(app: &AppHandle, id: &str, status: JobStatus, error: Option<String>) {
    let _ = app.emit(
        "download_status",
        StatusEvent {
            id: id.to_string(),
            status,
            error,
        },
    );
}

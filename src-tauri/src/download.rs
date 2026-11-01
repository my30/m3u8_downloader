use std::{
    collections::{HashMap, VecDeque},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use regex::Regex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};
use uuid::Uuid;

/// Maximum number of ffmpeg processes running at the same time.
pub const MAX_CONCURRENT: usize = 3;
/// Maximum number of log lines kept per job (ring buffer).
pub const MAX_LOG_LINES: usize = 500;
/// How long to wait for ffmpeg to finish gracefully after sending `q` before killing it.
const GRACEFUL_STOP_TIMEOUT: Duration = Duration::from_secs(5);

pub const ALLOWED_FORMATS: [&str; 3] = ["mp4", "mkv", "ts"];

// ── Types ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "PascalCase")]
pub enum JobStatus {
    Pending,
    Running,
    Done,
    Error,
    Canceled,
}

impl JobStatus {
    fn is_active(self) -> bool {
        matches!(self, JobStatus::Pending | JobStatus::Running)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DownloadJob {
    pub id: String,
    pub url: String,
    pub output_path: String,
    pub filename: String,
    pub format: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    pub headers: Vec<String>,
    pub status: JobStatus,
    pub progress_secs: Option<f64>,
    /// Expected output duration, if known (trim range, or VOD playlist duration).
    pub total_secs: Option<f64>,
    pub error: Option<String>,
    pub log_lines: VecDeque<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StartDownloadArgs {
    pub url: String,
    pub output_dir: String,
    pub filename: String,
    pub format: String,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    #[serde(default)]
    pub referer: Option<String>,
    /// Extra HTTP headers, one `Key: Value` per entry.
    #[serde(default)]
    pub headers: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProgressEvent {
    pub id: String,
    pub progress_secs: f64,
    pub total_secs: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LogEvent {
    pub id: String,
    pub line: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusEvent {
    pub id: String,
    pub status: JobStatus,
    pub error: Option<String>,
}

// ── Queue state ────────────────────────────────────────────────────────────
//
// Lock ordering: always take `jobs` before `children` to avoid deadlocks.

#[derive(Default)]
pub struct DownloadManager {
    pub jobs: Mutex<Vec<DownloadJob>>,
    /// Map of job_id -> child process handle (for cancellation)
    pub children: Mutex<HashMap<String, Child>>,
}

impl DownloadManager {
    /// Kill every running ffmpeg process. Called when the app exits so that no
    /// orphaned ffmpeg keeps downloading in the background.
    pub fn kill_all(&self) {
        let mut children = self.children.lock().unwrap();
        for (_, mut child) in children.drain() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

// ── ffmpeg path (sidecar) ──────────────────────────────────────────────────

/// Resolve the bundled ffmpeg sidecar.
///
/// Tauri places `externalBin` sidecars next to the main executable
/// (`Contents/MacOS/` on macOS, the install dir on Windows), and also copies
/// them into `target/<profile>/` during `tauri dev`.
fn ffmpeg_path() -> PathBuf {
    let name = if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    };
    if let Some(dir) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
    {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return candidate;
        }
    }
    // GUI apps on macOS don't inherit the shell PATH, so probe Homebrew locations.
    #[cfg(target_os = "macos")]
    for p in ["/opt/homebrew/bin/ffmpeg", "/usr/local/bin/ffmpeg"] {
        if Path::new(p).is_file() {
            return PathBuf::from(p);
        }
    }
    PathBuf::from(name)
}

// ── Validation helpers ─────────────────────────────────────────────────────

/// Parse `HH:MM:SS[.f]`, `MM:SS[.f]` or `S[.f]` into seconds.
pub fn parse_timestamp(s: &str) -> Option<f64> {
    let parts: Vec<&str> = s.trim().split(':').collect();
    if parts.is_empty() || parts.len() > 3 || parts.iter().any(|p| p.is_empty()) {
        return None;
    }
    let mut total = 0.0;
    for (i, p) in parts.iter().enumerate() {
        let last = i == parts.len() - 1;
        if !p.chars().all(|c| c.is_ascii_digit() || (last && c == '.')) {
            return None;
        }
        let v: f64 = p.parse().ok()?;
        if i > 0 && v >= 60.0 {
            return None;
        }
        total = total * 60.0 + v;
    }
    Some(total)
}

/// Make a user-supplied filename safe: no path separators, no reserved
/// characters, no traversal, no Windows reserved device names.
pub fn sanitize_filename(name: &str) -> Result<String, String> {
    let cleaned: String = name
        .trim()
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let cleaned = cleaned
        .trim_matches(|c: char| c == '.' || c == ' ')
        .to_string();
    if cleaned.is_empty() {
        return Err("Filename is empty or invalid.".into());
    }
    const RESERVED: [&str; 22] = [
        "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
        "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
    ];
    let stem = cleaned.split('.').next().unwrap_or("").to_ascii_uppercase();
    if RESERVED.contains(&stem.as_str()) {
        return Ok(format!("_{cleaned}"));
    }
    Ok(cleaned)
}

fn validate_header(h: &str) -> Result<(), String> {
    if h.contains('\r') || h.contains('\n') {
        return Err(format!("Header contains a line break: {h:?}"));
    }
    match h.split_once(':') {
        Some((k, _)) if !k.trim().is_empty() && !k.contains(' ') => Ok(()),
        _ => Err(format!("Header must look like 'Name: value': {h:?}")),
    }
}

fn non_empty(s: Option<String>) -> Option<String> {
    s.map(|v| v.trim().to_string()).filter(|v| !v.is_empty())
}

/// Validate the user's request and turn it into a job.
pub fn job_from_args(args: StartDownloadArgs) -> Result<DownloadJob, String> {
    let url = args.url.trim().to_string();
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("http://") || lower.starts_with("https://")) {
        return Err("URL must start with http:// or https://".into());
    }

    let format = args.format.to_ascii_lowercase();
    if !ALLOWED_FORMATS.contains(&format.as_str()) {
        return Err(format!("Unsupported format '{format}'."));
    }

    let dir = PathBuf::from(args.output_dir.trim());
    if !dir.is_dir() {
        return Err(format!(
            "Output directory does not exist: {}",
            dir.display()
        ));
    }
    let filename = sanitize_filename(&args.filename)?;
    let output = dir.join(format!("{filename}.{format}"));
    if output.exists() {
        return Err(format!("File already exists: {}", output.display()));
    }

    let start_time = non_empty(args.start_time);
    let end_time = non_empty(args.end_time);
    let start = match &start_time {
        Some(s) => Some(parse_timestamp(s).ok_or("Invalid start time. Use HH:MM:SS or seconds.")?),
        None => None,
    };
    let end = match &end_time {
        Some(s) => Some(parse_timestamp(s).ok_or("Invalid end time. Use HH:MM:SS or seconds.")?),
        None => None,
    };
    if let (Some(s), Some(e)) = (start, end) {
        if e <= s {
            return Err("End time must be after start time.".into());
        }
    }
    let total_secs = end.map(|e| e - start.unwrap_or(0.0));

    let user_agent = non_empty(args.user_agent);
    let referer = non_empty(args.referer);
    for v in [&user_agent, &referer].into_iter().flatten() {
        if v.contains('\r') || v.contains('\n') {
            return Err("User-Agent / Referer must not contain line breaks.".into());
        }
    }
    let headers: Vec<String> = args
        .headers
        .into_iter()
        .map(|h| h.trim().to_string())
        .filter(|h| !h.is_empty())
        .collect();
    for h in &headers {
        validate_header(h)?;
    }

    Ok(DownloadJob {
        id: Uuid::new_v4().to_string(),
        url,
        output_path: output.to_string_lossy().into_owned(),
        filename,
        format,
        start_time,
        end_time,
        user_agent,
        referer,
        headers,
        status: JobStatus::Pending,
        progress_secs: None,
        total_secs,
        error: None,
        log_lines: VecDeque::new(),
    })
}

// ── Build ffmpeg argument list ─────────────────────────────────────────────

pub fn build_ffmpeg_args(job: &DownloadJob) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        // Machine-readable progress on stdout (newline-terminated key=value),
        // instead of the `\r`-terminated human stats line on stderr.
        "-nostats".into(),
        "-progress".into(),
        "pipe:1".into(),
        // Only network protocols needed for HLS; blocks file:/concat: etc.
        "-protocol_whitelist".into(),
        "http,https,tcp,tls,crypto,httpproxy,data".into(),
    ];

    // HTTP options (input options, also applied to segment/key requests by the HLS demuxer)
    if let Some(ua) = &job.user_agent {
        args.extend(["-user_agent".into(), ua.clone()]);
    }
    if let Some(r) = &job.referer {
        args.extend(["-referer".into(), r.clone()]);
    }
    if !job.headers.is_empty() {
        let joined: String = job.headers.iter().map(|h| format!("{h}\r\n")).collect();
        args.extend(["-headers".into(), joined]);
    }

    // Both -ss and -to as *input* options, so -to is an absolute position in the
    // source rather than a duration counted from -ss.
    if let Some(st) = &job.start_time {
        args.extend(["-ss".into(), st.clone()]);
    }
    if let Some(et) = &job.end_time {
        args.extend(["-to".into(), et.clone()]);
    }

    args.extend(["-i".into(), job.url.clone()]);

    // Stream copy. The mp4/mkv muxers insert aac_adtstoasc automatically when
    // needed, so we don't force it (forcing it fails on non-AAC audio).
    args.extend(["-c".into(), "copy".into()]);

    // Never overwrite silently (existence is also checked up-front).
    args.push("-n".into());
    args.push(job.output_path.clone());
    args
}

// ── Output parsing ─────────────────────────────────────────────────────────

/// Parse an `out_time_us=<microseconds>` line from `-progress` output.
pub fn parse_progress_line(line: &str) -> Option<f64> {
    let v = line.trim().strip_prefix("out_time_us=")?;
    let us: i64 = v.parse().ok()?;
    (us >= 0).then(|| us as f64 / 1_000_000.0)
}

/// Parse the input duration from ffmpeg's stderr (`Duration: 00:01:02.50, ...`).
/// Live streams report `Duration: N/A` and yield `None`.
pub fn parse_duration_line(line: &str) -> Option<f64> {
    static RE: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r"Duration:\s*(\d+):(\d{2}):(\d{2}(?:\.\d+)?)").unwrap());
    let caps = RE.captures(line)?;
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
    manager
        .jobs
        .lock()
        .unwrap()
        .retain(|j| j.status.is_active());
}

#[tauri::command]
pub fn cancel_download(id: String, manager: tauri::State<Arc<DownloadManager>>, app: AppHandle) {
    let manager = manager.inner().clone();
    let canceled = {
        let mut jobs = manager.jobs.lock().unwrap();
        match jobs.iter_mut().find(|j| j.id == id) {
            Some(job) if job.status.is_active() => {
                job.status = JobStatus::Canceled;
                // Ask ffmpeg to stop gracefully so the output file gets finalized
                // (important for mp4, whose index is written at the end).
                let mut children = manager.children.lock().unwrap();
                if let Some(child) = children.get_mut(&id) {
                    if let Some(stdin) = child.stdin.as_mut() {
                        let _ = stdin.write_all(b"q");
                        let _ = stdin.flush();
                    }
                }
                true
            }
            _ => false,
        }
    };
    if !canceled {
        return;
    }
    emit_status(&app, &id, JobStatus::Canceled, None);

    // Fallback: hard kill if ffmpeg hasn't exited after the timeout.
    // The runner thread is responsible for wait()-ing the child.
    let mgr = manager.clone();
    std::thread::spawn(move || {
        std::thread::sleep(GRACEFUL_STOP_TIMEOUT);
        if let Some(child) = mgr.children.lock().unwrap().get_mut(&id) {
            let _ = child.kill();
        }
    });
    // A slot may have been freed (pending job canceled) — start the next one.
    pump(&manager, &app);
}

#[tauri::command]
pub fn start_download(
    args: StartDownloadArgs,
    manager: tauri::State<Arc<DownloadManager>>,
    app: AppHandle,
) -> Result<String, String> {
    let job = job_from_args(args)?;
    let id = job.id.clone();
    {
        let mut jobs = manager.jobs.lock().unwrap();
        if jobs
            .iter()
            .any(|j| j.status.is_active() && j.output_path == job.output_path)
        {
            return Err(format!(
                "Another download is already writing to {}",
                job.output_path
            ));
        }
        jobs.push(job);
    }
    emit_status(&app, &id, JobStatus::Pending, None);
    pump(manager.inner(), &app);
    Ok(id)
}

// ── Scheduler ──────────────────────────────────────────────────────────────

/// Start pending jobs while fewer than `MAX_CONCURRENT` are running.
fn pump(manager: &Arc<DownloadManager>, app: &AppHandle) {
    let to_start: Vec<DownloadJob> = {
        let mut jobs = manager.jobs.lock().unwrap();
        let running = jobs
            .iter()
            .filter(|j| j.status == JobStatus::Running)
            .count();
        let free = MAX_CONCURRENT.saturating_sub(running);
        jobs.iter_mut()
            .filter(|j| j.status == JobStatus::Pending)
            .take(free)
            .map(|j| {
                // Marked Running under the lock, so a concurrent cancel can't be lost.
                j.status = JobStatus::Running;
                j.clone()
            })
            .collect()
    };
    for job in to_start {
        emit_status(app, &job.id, JobStatus::Running, None);
        let manager = manager.clone();
        let app = app.clone();
        std::thread::spawn(move || {
            run_ffmpeg(job, &manager, &app);
            pump(&manager, &app);
        });
    }
}

// ── ffmpeg runner (runs in a background thread) ────────────────────────────

fn run_ffmpeg(job: DownloadJob, manager: &Arc<DownloadManager>, app: &AppHandle) {
    let job_id = job.id.clone();
    let mut cmd = Command::new(ffmpeg_path());
    cmd.args(build_ffmpeg_args(&job))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(e) => {
            finish(
                manager,
                app,
                &job_id,
                Err(format!("Failed to start ffmpeg: {e}")),
            );
            return;
        }
    };
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    // Register the child; if the job was canceled in the meantime, stop right away.
    {
        let jobs = manager.jobs.lock().unwrap();
        let canceled = jobs
            .iter()
            .find(|j| j.id == job_id)
            .is_none_or(|j| j.status == JobStatus::Canceled);
        if canceled {
            drop(jobs);
            let _ = child.kill();
            let _ = child.wait();
            return;
        }
        manager
            .children
            .lock()
            .unwrap()
            .insert(job_id.clone(), child);
    }

    // stderr → log lines + input duration (separate thread so neither pipe blocks)
    let stderr_thread = {
        let manager = manager.clone();
        let app = app.clone();
        let job_id = job_id.clone();
        std::thread::spawn(move || {
            let mut last_line = None;
            for line in BufReader::new(stderr).lines() {
                let Ok(line) = line else { break };
                let duration = parse_duration_line(&line);
                {
                    let mut jobs = manager.jobs.lock().unwrap();
                    if let Some(j) = jobs.iter_mut().find(|j| j.id == job_id) {
                        if j.total_secs.is_none() {
                            j.total_secs = duration.map(|d| {
                                match j.start_time.as_deref().and_then(parse_timestamp) {
                                    Some(s) => (d - s).max(0.0),
                                    None => d,
                                }
                            });
                        }
                        j.log_lines.push_back(line.clone());
                        while j.log_lines.len() > MAX_LOG_LINES {
                            j.log_lines.pop_front();
                        }
                    }
                }
                let _ = app.emit(
                    "download_log",
                    LogEvent {
                        id: job_id.clone(),
                        line: line.clone(),
                    },
                );
                if !line.trim().is_empty() {
                    last_line = Some(line);
                }
            }
            last_line
        })
    };

    // stdout → progress
    for line in BufReader::new(stdout).lines() {
        let Ok(line) = line else { break };
        if let Some(secs) = parse_progress_line(&line) {
            let total = {
                let mut jobs = manager.jobs.lock().unwrap();
                jobs.iter_mut().find(|j| j.id == job_id).and_then(|j| {
                    j.progress_secs = Some(secs);
                    j.total_secs
                })
            };
            let _ = app.emit(
                "download_progress",
                ProgressEvent {
                    id: job_id.clone(),
                    progress_secs: secs,
                    total_secs: total,
                },
            );
        }
    }

    let last_stderr = stderr_thread.join().ok().flatten();
    let exit_status = {
        let child = manager.children.lock().unwrap().remove(&job_id);
        child.and_then(|mut c| c.wait().ok())
    };

    let result = match exit_status {
        Some(s) if s.success() => Ok(()),
        _ => Err(match last_stderr {
            Some(l) => format!("ffmpeg failed: {l}"),
            None => "ffmpeg exited with a non-zero status code.".into(),
        }),
    };
    finish(manager, app, &job_id, result);
}

/// Record the final status of a job (never overwriting `Canceled`).
fn finish(manager: &Arc<DownloadManager>, app: &AppHandle, id: &str, result: Result<(), String>) {
    let mut jobs = manager.jobs.lock().unwrap();
    let Some(j) = jobs.iter_mut().find(|j| j.id == id) else {
        return;
    };
    if j.status == JobStatus::Canceled {
        return;
    }
    match result {
        Ok(()) => {
            j.status = JobStatus::Done;
            if let Some(t) = j.total_secs {
                j.progress_secs = Some(t);
            }
            drop(jobs);
            emit_status(app, id, JobStatus::Done, None);
        }
        Err(msg) => {
            j.status = JobStatus::Error;
            j.error = Some(msg.clone());
            drop(jobs);
            emit_status(app, id, JobStatus::Error, Some(msg));
        }
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

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn args(dir: &Path) -> StartDownloadArgs {
        StartDownloadArgs {
            url: "https://example.com/a.m3u8".into(),
            output_dir: dir.to_string_lossy().into_owned(),
            filename: "video".into(),
            format: "mp4".into(),
            start_time: None,
            end_time: None,
            user_agent: None,
            referer: None,
            headers: vec![],
        }
    }

    fn pos(v: &[String], s: &str) -> usize {
        v.iter()
            .position(|a| a == s)
            .unwrap_or_else(|| panic!("{s} not in {v:?}"))
    }

    #[test]
    fn timestamps() {
        assert_eq!(parse_timestamp("90"), Some(90.0));
        assert_eq!(parse_timestamp("1:30"), Some(90.0));
        assert_eq!(parse_timestamp("01:01:30.5"), Some(3690.5));
        assert_eq!(parse_timestamp("1:60"), None);
        assert_eq!(parse_timestamp("1::2"), None);
        assert_eq!(parse_timestamp("1.5:00"), None);
        assert_eq!(parse_timestamp("abc"), None);
    }

    #[test]
    fn filenames() {
        assert_eq!(
            sanitize_filename("../../etc/passwd").unwrap(),
            "_.._etc_passwd"
        );
        assert_eq!(sanitize_filename("a:b*c").unwrap(), "a_b_c");
        assert_eq!(sanitize_filename("con").unwrap(), "_con");
        assert!(sanitize_filename(" .. ").is_err());
    }

    #[test]
    fn ss_and_to_are_input_options() {
        let dir = std::env::temp_dir();
        let mut a = args(&dir);
        a.filename = format!("t-{}", Uuid::new_v4());
        a.start_time = Some("00:01:00".into());
        a.end_time = Some("00:02:30".into());
        let job = job_from_args(a).unwrap();
        assert_eq!(job.total_secs, Some(90.0));
        let v = build_ffmpeg_args(&job);
        let i = pos(&v, "-i");
        assert!(pos(&v, "-ss") < i && pos(&v, "-to") < i);
        assert!(!v.iter().any(|a| a == "-bsf:a"));
        assert!(v.contains(&"-n".to_string()) && !v.contains(&"-y".to_string()));
        assert_eq!(
            v.last().unwrap(),
            &dir.join(format!("{}.mp4", job.filename)).to_string_lossy()
        );
    }

    #[test]
    fn rejects_bad_input() {
        let dir = std::env::temp_dir();
        let mut a = args(&dir);
        a.url = "file:///etc/passwd".into();
        assert!(job_from_args(a).is_err());

        let mut a = args(&dir);
        a.start_time = Some("10".into());
        a.end_time = Some("5".into());
        assert!(job_from_args(a).is_err());

        let mut a = args(&dir);
        a.format = "exe".into();
        assert!(job_from_args(a).is_err());

        let mut a = args(&dir);
        a.headers = vec!["Cookie: a=b\r\nX-Evil: 1".into()];
        assert!(job_from_args(a).is_err());

        let mut a = args(&dir);
        a.output_dir = "/definitely/not/here".into();
        assert!(job_from_args(a).is_err());
    }

    #[test]
    fn rejects_existing_file() {
        let dir = std::env::temp_dir();
        let name = format!("exists-{}", Uuid::new_v4());
        let path = dir.join(format!("{name}.mp4"));
        std::fs::write(&path, b"x").unwrap();
        let mut a = args(&dir);
        a.filename = name;
        let r = job_from_args(a);
        std::fs::remove_file(&path).unwrap();
        assert!(r.unwrap_err().contains("already exists"));
    }

    #[test]
    fn headers_are_passed() {
        let dir = std::env::temp_dir();
        let mut a = args(&dir);
        a.filename = format!("h-{}", Uuid::new_v4());
        a.user_agent = Some("UA/1".into());
        a.referer = Some("https://ref.example/".into());
        a.headers = vec!["Cookie: a=b".into(), "  ".into(), "X-Token: t".into()];
        let job = job_from_args(a).unwrap();
        let v = build_ffmpeg_args(&job);
        let i = pos(&v, "-i");
        assert!(pos(&v, "-user_agent") < i && pos(&v, "-referer") < i);
        assert_eq!(v[pos(&v, "-headers") + 1], "Cookie: a=b\r\nX-Token: t\r\n");
    }

    #[test]
    fn progress_and_duration_parsing() {
        assert_eq!(parse_progress_line("out_time_us=20015600"), Some(20.0156));
        assert_eq!(parse_progress_line("out_time_us=-56778"), None);
        assert_eq!(parse_progress_line("out_time_us=N/A"), None);
        assert_eq!(parse_progress_line("total_size=1"), None);
        assert_eq!(
            parse_duration_line("  Duration: 00:01:02.50, start: 1.4, bitrate: N/A"),
            Some(62.5)
        );
        assert_eq!(parse_duration_line("  Duration: N/A, start: 0"), None);
    }

    /// End-to-end check against a real ffmpeg (skipped if ffmpeg isn't installed):
    /// generates a local 20 s HLS stream, then verifies trimming semantics and
    /// that progress arrives as newline-terminated lines on stdout.
    #[test]
    fn real_ffmpeg_trim_and_progress() {
        if Command::new("ffmpeg").arg("-version").output().is_err() {
            eprintln!("ffmpeg not found; skipping");
            return;
        }
        let dir = std::env::temp_dir().join(format!("m3u8-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let playlist = dir.join("s.m3u8");
        let ok = Command::new("ffmpeg")
            .args([
                "-v",
                "error",
                "-f",
                "lavfi",
                "-i",
                "testsrc=duration=20:size=160x120:rate=25",
            ])
            .args([
                "-f",
                "lavfi",
                "-i",
                "sine=duration=20",
                "-c:v",
                "libx264",
                "-g",
                "25",
            ])
            .args([
                "-c:a",
                "aac",
                "-f",
                "hls",
                "-hls_time",
                "1",
                "-hls_list_size",
                "0",
                "-y",
            ])
            .arg(&playlist)
            .status()
            .unwrap()
            .success();
        assert!(ok, "failed to generate test stream");

        let mut a = args(&dir);
        a.start_time = Some("5".into());
        a.end_time = Some("8".into());
        let mut job = job_from_args(a).unwrap();
        // Local file input for the test only (the real app requires http/https).
        job.url = playlist.to_string_lossy().into_owned();
        let mut v = build_ffmpeg_args(&job);
        let wl = pos(&v, "-protocol_whitelist");
        v[wl + 1] = "file,crypto,data".into();

        let out = Command::new("ffmpeg").args(&v).output().unwrap();
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(stdout.lines().filter_map(parse_progress_line).count() >= 1);
        assert!(!String::from_utf8_lossy(&out.stderr).contains('\r'));

        let probe = Command::new("ffprobe")
            .args([
                "-v",
                "error",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
            ])
            .arg(&job.output_path)
            .output()
            .unwrap();
        let dur: f64 = String::from_utf8_lossy(&probe.stdout)
            .trim()
            .parse()
            .unwrap();
        // 5s→8s should be ~3 s (keyframe snapping allows some slack), not 8 s.
        assert!((2.0..=4.5).contains(&dur), "duration was {dur}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

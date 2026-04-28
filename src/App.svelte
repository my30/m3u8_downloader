<script lang="ts">
  import "./app.css";
  import DownloadForm from "./components/DownloadForm.svelte";
  import QueuePanel from "./components/QueuePanel.svelte";
  import {
    getQueue,
    onProgress,
    onStatusChange,
    type DownloadJob,
  } from "./lib/tauri";
  import { onMount } from "svelte";

  let jobs = $state<DownloadJob[]>([]);

  onMount(async () => {
    // Load existing queue (e.g. after hot reload)
    try {
      jobs = await getQueue();
    } catch {}

    // Listen for progress updates from running ffmpeg processes
    const unlistenProgress = await onProgress((e) => {
      jobs = jobs.map((j) =>
        j.id === e.id
          ? { ...j, progress_secs: e.progress_secs, log_lines: [...j.log_lines, e.log_line] }
          : j
      );
    });

    // Listen for status changes (Done / Error / Canceled)
    const unlistenStatus = await onStatusChange((e) => {
      jobs = jobs.map((j) =>
        j.id === e.id ? { ...j, status: e.status, error: e.error } : j
      );
    });

    return () => {
      unlistenProgress();
      unlistenStatus();
    };
  });

  // When a new job is started from the form, refresh the full queue
  async function handleJobAdded() {
    try {
      jobs = await getQueue();
    } catch {}
  }
</script>

<div class="app-shell">
  <header>
    <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="color:#6c63ff">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/>
      <polyline points="7 10 12 15 17 10"/>
      <line x1="12" y1="15" x2="12" y2="3"/>
    </svg>
    <h1>M3U8 Downloader</h1>
    <span class="subtitle">ffmpeg-powered · HLS stream capture</span>
  </header>

  <div class="content">
    <DownloadForm onqueued={handleJobAdded} />
    <QueuePanel bind:jobs />
  </div>
</div>

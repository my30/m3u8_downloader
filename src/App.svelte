<script lang="ts">
  import "./app.css";
  import DownloadForm from "./components/DownloadForm.svelte";
  import QueuePanel from "./components/QueuePanel.svelte";
  import {
    getQueue,
    onLog,
    onProgress,
    onStatusChange,
    MAX_LOG_LINES,
    type DownloadJob,
  } from "./lib/tauri";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";

  const REPO_URL = "https://github.com/my30/m3u8_downloader";

  let jobs = $state<DownloadJob[]>([]);

  async function refresh() {
    try {
      jobs = await getQueue();
    } catch {}
  }

  function update(id: string, fn: (j: DownloadJob) => DownloadJob) {
    const i = jobs.findIndex((j) => j.id === id);
    if (i === -1) {
      // Event for a job we don't know yet (e.g. emitted before start_download returned)
      refresh();
      return;
    }
    jobs[i] = fn(jobs[i]);
  }

  // onMount must be synchronous for its cleanup function to be used.
  onMount(() => {
    const unlisteners: Promise<UnlistenFn>[] = [
      onProgress((e) =>
        update(e.id, (j) => ({ ...j, progress_secs: e.progress_secs, total_secs: e.total_secs ?? j.total_secs }))
      ),
      onLog((e) =>
        update(e.id, (j) => ({ ...j, log_lines: [...j.log_lines, e.line].slice(-MAX_LOG_LINES) }))
      ),
      onStatusChange((e) => update(e.id, (j) => ({ ...j, status: e.status, error: e.error }))),
    ];
    refresh();
    return () => {
      unlisteners.forEach((p) => p.then((un) => un()));
    };
  });

  function openRepo(e: MouseEvent) {
    e.preventDefault();
    openUrl(REPO_URL).catch(() => {});
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
    <a
      href={REPO_URL}
      onclick={openRepo}
      class="github-link"
      title="View on GitHub"
    >
      <svg width="16" height="16" viewBox="0 0 24 24" fill="currentColor" aria-hidden="true">
        <path d="M12 2C6.477 2 2 6.484 2 12.017c0 4.425 2.865 8.18 6.839 9.504.5.092.682-.217.682-.483 0-.237-.008-.868-.013-1.703-2.782.605-3.369-1.343-3.369-1.343-.454-1.158-1.11-1.466-1.11-1.466-.908-.62.069-.608.069-.608 1.003.07 1.531 1.032 1.531 1.032.892 1.53 2.341 1.088 2.91.832.092-.647.35-1.088.636-1.338-2.22-.253-4.555-1.113-4.555-4.951 0-1.093.39-1.988 1.029-2.688-.103-.253-.446-1.272.098-2.65 0 0 .84-.27 2.75 1.026A9.564 9.564 0 0 1 12 6.844a9.59 9.59 0 0 1 2.504.337c1.909-1.296 2.747-1.027 2.747-1.027.546 1.379.202 2.398.1 2.651.64.7 1.028 1.595 1.028 2.688 0 3.848-2.339 4.695-4.566 4.943.359.309.678.92.678 1.855 0 1.338-.012 2.419-.012 2.747 0 .268.18.58.688.482A10.02 10.02 0 0 0 22 12.017C22 6.484 17.522 2 12 2z"/>
      </svg>
      <span>my30</span>
    </a>
  </header>

  <div class="content">
    <DownloadForm onqueued={refresh} />
    <QueuePanel bind:jobs />
  </div>
</div>

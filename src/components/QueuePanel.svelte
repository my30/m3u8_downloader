<script lang="ts">
  import { cancelDownload, clearCompleted, type DownloadJob } from "../lib/tauri";

  let { jobs = $bindable() }: { jobs: DownloadJob[] } = $props();

  let expandedLogs = $state<Set<string>>(new Set());

  function toggleLog(id: string) {
    const next = new Set(expandedLogs);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedLogs = next;
  }

  async function cancel(id: string) {
    // Status update arrives via the download_status event.
    await cancelDownload(id);
  }

  async function clearDone() {
    await clearCompleted();
    jobs = jobs.filter((j) => j.status === "Running" || j.status === "Pending");
  }

  function badgeClass(status: DownloadJob["status"]): string {
    const map: Record<DownloadJob["status"], string> = {
      Pending: "badge-pending",
      Running: "badge-running",
      Done: "badge-done",
      Error: "badge-error",
      Canceled: "badge-canceled",
    };
    return "badge " + map[status];
  }

  function formatSecs(secs: number | null): string {
    if (secs === null) return "";
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = Math.floor(secs % 60);
    return [h, m, s].map((v) => String(v).padStart(2, "0")).join(":");
  }

  /** Percentage if the total duration is known, otherwise null (indeterminate). */
  function percent(job: DownloadJob): number | null {
    if (job.status === "Done") return 100;
    if (!job.total_secs || job.progress_secs === null) return null;
    return Math.min(100, (job.progress_secs / job.total_secs) * 100);
  }

  const hasFinished = $derived(
    jobs.some((j) => j.status === "Done" || j.status === "Error" || j.status === "Canceled")
  );
</script>

<div class="card">
  <div class="card-title" style="display:flex; align-items:center; justify-content:space-between;">
    <span>Download Queue</span>
    {#if hasFinished}
      <button class="btn-secondary" style="font-size:11px; padding:3px 10px;" onclick={clearDone}>
        Clear finished
      </button>
    {/if}
  </div>

  {#if jobs.length === 0}
    <div class="queue-empty">No downloads yet. Add a URL above to get started.</div>
  {:else}
    {#each jobs as job (job.id)}
      <div class="job-item">
        <div class="job-header">
          <span class="job-filename">{job.filename}.{job.format}</span>
          <span class={badgeClass(job.status)}>{job.status}</span>
          {#if job.status === "Running" || job.status === "Pending"}
            <button class="btn-danger" onclick={() => cancel(job.id)}>Cancel</button>
          {/if}
        </div>

        <div class="job-url" title={job.url}>{job.url}</div>
        <div class="job-url" title={job.output_path}>→ {job.output_path}</div>

        {#if job.start_time || job.end_time}
          <div class="job-meta">
            {#if job.start_time}Start: {job.start_time}{/if}
            {#if job.start_time && job.end_time} · {/if}
            {#if job.end_time}End: {job.end_time}{/if}
          </div>
        {/if}

        {#if job.status === "Running" || job.status === "Done"}
          {@const pct = percent(job)}
          <div style="display:flex; align-items:center; gap:8px; margin-top:4px;">
            <div class="progress-wrap" style="flex:1">
              <div
                class="progress-fill {pct === null ? 'indeterminate' : ''}"
                style="width: {pct ?? 0}%"
              ></div>
            </div>
            {#if job.progress_secs !== null}
              <span class="job-time">
                {formatSecs(job.progress_secs)}{#if job.total_secs} / {formatSecs(job.total_secs)}{/if}
              </span>
            {/if}
          </div>
        {/if}

        {#if job.status === "Pending"}
          <div class="job-meta">Waiting for a free slot…</div>
        {/if}

        {#if job.status === "Error" && job.error}
          <div class="form-error" style="margin-top:6px; font-size:12px;">{job.error}</div>
        {/if}

        {#if job.log_lines.length > 0}
          <button class="log-toggle" onclick={() => toggleLog(job.id)}>
            {expandedLogs.has(job.id) ? "Hide log" : "Show log"} (last {job.log_lines.length} lines)
          </button>
          {#if expandedLogs.has(job.id)}
            <div class="log-box">{job.log_lines.join("\n")}</div>
          {/if}
        {/if}
      </div>
    {/each}
  {/if}
</div>

<style>
  @keyframes indeterminate {
    0%   { transform: translateX(-100%); }
    100% { transform: translateX(250%); }
  }

  :global(.progress-fill.indeterminate) {
    width: 40% !important;
    animation: indeterminate 1.4s ease-in-out infinite;
  }
</style>

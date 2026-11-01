<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { downloadDir } from "@tauri-apps/api/path";
  import { onMount } from "svelte";
  import {
    parseTimestamp,
    startDownload,
    type OutputFormat,
    type StartDownloadArgs,
  } from "../lib/tauri";

  let { onqueued }: { onqueued?: () => void } = $props();

  let url = $state("");
  let startTime = $state("");
  let endTime = $state("");
  let outputDir = $state("");
  let filename = $state("");
  let format = $state<OutputFormat>("mp4");
  let userAgent = $state("");
  let referer = $state("");
  let headersText = $state("");
  let showAdvanced = $state(false);
  let submitting = $state(false);
  let error = $state("");

  const formats: OutputFormat[] = ["mp4", "mkv", "ts"];

  onMount(async () => {
    try { outputDir = await downloadDir(); } catch {}
  });

  async function pickDirectory() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      outputDir = selected;
    }
  }

  async function handleSubmit() {
    error = "";
    const trimmedUrl = url.trim();
    const trimmedDir = outputDir.trim();
    const trimmedFilename = filename.trim();

    if (!trimmedUrl) { error = "URL is required."; return; }
    if (!/^https?:\/\//i.test(trimmedUrl)) { error = "URL must start with http:// or https://"; return; }
    if (!trimmedDir) { error = "Output directory is required."; return; }
    if (!trimmedFilename) { error = "Filename is required."; return; }

    const start = startTime.trim() ? parseTimestamp(startTime) : null;
    const end = endTime.trim() ? parseTimestamp(endTime) : null;
    if (startTime.trim() && start === null) { error = "Invalid start time. Use HH:MM:SS or seconds."; return; }
    if (endTime.trim() && end === null) { error = "Invalid end time. Use HH:MM:SS or seconds."; return; }
    if (start !== null && end !== null && end <= start) { error = "End time must be after start time."; return; }

    const headers = headersText.split("\n").map((h) => h.trim()).filter(Boolean);
    const badHeader = headers.find((h) => !/^[^\s:]+:/.test(h));
    if (badHeader) { error = `Header must look like "Name: value": ${badHeader}`; return; }

    const args: StartDownloadArgs = {
      url: trimmedUrl,
      output_dir: trimmedDir,
      filename: trimmedFilename,
      format,
      start_time: startTime.trim() || null,
      end_time: endTime.trim() || null,
      user_agent: userAgent.trim() || null,
      referer: referer.trim() || null,
      headers,
    };

    try {
      submitting = true;
      await startDownload(args);
      // Reset per-video fields on success (keep dir, format and headers)
      url = "";
      startTime = "";
      endTime = "";
      filename = "";
      onqueued?.();
    } catch (e) {
      error = String(e);
    } finally {
      submitting = false;
    }
  }
</script>

<div class="card">
  <div class="card-title">New Download</div>

  <div class="field">
    <label for="url-input">Stream URL</label>
    <input
      id="url-input"
      type="url"
      bind:value={url}
      placeholder="https://example.com/stream/index.m3u8"
      autocomplete="off"
      spellcheck="false"
    />
  </div>

  <div class="input-row" style="align-items: flex-start;">
    <div class="field" style="flex:1">
      <label for="start-time">
        Start Time <span class="optional">(optional)</span>
      </label>
      <input
        id="start-time"
        type="text"
        bind:value={startTime}
        placeholder="e.g. 00:01:30"
      />
    </div>
    <div class="field" style="flex:1">
      <label for="end-time">
        End Time <span class="optional">(optional)</span>
      </label>
      <input
        id="end-time"
        type="text"
        bind:value={endTime}
        placeholder="e.g. 00:05:00"
      />
    </div>
  </div>
  <div class="hint">End time is a position in the source, not a duration. With stream copy, the start snaps to the nearest keyframe (can be off by a few seconds).</div>

  <div class="field">
    <label for="output-dir">Output Directory</label>
    <div class="path-row">
      <input
        id="output-dir"
        type="text"
        bind:value={outputDir}
        placeholder="/Users/you/Downloads"
        spellcheck="false"
      />
      <button class="btn-secondary" onclick={pickDirectory}>Browse…</button>
    </div>
  </div>

  <div class="input-row">
    <div class="field" style="flex:1">
      <label for="filename">Filename <span class="optional">(no extension)</span></label>
      <input
        id="filename"
        type="text"
        bind:value={filename}
        placeholder="my-video"
        spellcheck="false"
      />
    </div>
    <div class="field">
      <!-- svelte-ignore a11y_label_has_associated_control -->
      <label>Format</label>
      <div class="format-grid" role="group" aria-label="Output format">
        {#each formats as f}
          <button
            class="format-btn {format === f ? 'active' : ''}"
            onclick={() => (format = f)}
            aria-pressed={format === f}
          >{f}</button>
        {/each}
      </div>
    </div>
  </div>

  <button class="log-toggle" onclick={() => (showAdvanced = !showAdvanced)} aria-expanded={showAdvanced}>
    {showAdvanced ? "▾" : "▸"} HTTP headers <span class="optional">(for sites that need Referer / cookies)</span>
  </button>
  {#if showAdvanced}
    <div class="input-row">
      <div class="field" style="flex:1">
        <label for="referer">Referer</label>
        <input id="referer" type="text" bind:value={referer} placeholder="https://example.com/watch/123" spellcheck="false" />
      </div>
      <div class="field" style="flex:1">
        <label for="user-agent">User-Agent</label>
        <input id="user-agent" type="text" bind:value={userAgent} placeholder="Mozilla/5.0 …" spellcheck="false" />
      </div>
    </div>
    <div class="field">
      <label for="headers">Extra headers <span class="optional">(one "Name: value" per line)</span></label>
      <textarea id="headers" rows="3" bind:value={headersText} placeholder="Cookie: session=abc" spellcheck="false"></textarea>
    </div>
  {/if}

  {#if error}
    <div class="form-error">{error}</div>
  {/if}

  <button
    class="btn-primary"
    onclick={handleSubmit}
    disabled={submitting}
  >
    {submitting ? "Adding to queue…" : "Add to Queue"}
  </button>
</div>

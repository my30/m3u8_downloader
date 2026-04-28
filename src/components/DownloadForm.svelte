<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import {
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
  let submitting = $state(false);
  let error = $state("");

  const formats: OutputFormat[] = ["mp4", "mkv", "ts"];

  async function pickDirectory() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      outputDir = selected;
    }
  }

  function validateTime(val: string): boolean {
    if (!val) return true; // optional
    // Accept HH:MM:SS, MM:SS, or plain seconds
    return /^(\d+:\d{2}:\d{2}|\d+:\d{2}|\d+(\.\d+)?)$/.test(val.trim());
  }

  async function handleSubmit() {
    error = "";
    const trimmedUrl = url.trim();
    const trimmedDir = outputDir.trim();
    const trimmedFilename = filename.trim();

    if (!trimmedUrl) { error = "URL is required."; return; }
    if (!trimmedDir) { error = "Output directory is required."; return; }
    if (!trimmedFilename) { error = "Filename is required."; return; }
    if (startTime && !validateTime(startTime)) {
      error = "Invalid start time. Use HH:MM:SS or seconds."; return;
    }
    if (endTime && !validateTime(endTime)) {
      error = "Invalid end time. Use HH:MM:SS or seconds."; return;
    }

    const args: StartDownloadArgs = {
      url: trimmedUrl,
      output_dir: trimmedDir,
      filename: trimmedFilename,
      format,
      start_time: startTime.trim() || null,
      end_time: endTime.trim() || null,
    };

    try {
      submitting = true;
      await startDownload(args);
      // Reset form on success
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

  <div class="input-row">
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

<script lang="ts">
  import { open } from "@tauri-apps/plugin-dialog";
  import { api } from "./api";
  let { onDone }: { onDone: () => void } = $props();

  let busy = $state(false);
  let error = $state("");

  async function autoDownload() {
    busy = true; error = "";
    try {
      await api.downloadEngine();
      onDone();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }

  async function pick() {
    const path = await open({ multiple: false, title: "Select CloakBrowser binary" });
    if (typeof path === "string") {
      await api.setEnginePath(path);
      onDone();
    }
  }
</script>

<div class="onboard">
  <h2>Get the CloakBrowser engine</h2>
  <p>
    rustcloak does not ship the browser (license: no redistribution). It can download
    the official macOS build (~hundreds of MB) from cloakbrowser.dev for you, or you
    can point to an existing binary.
  </p>
  <button class="primary" onclick={autoDownload} disabled={busy}>
    {busy ? "Downloading… (this can take a few minutes)" : "Download automatically"}
  </button>
  <button onclick={pick} disabled={busy}>Choose existing binary…</button>
  {#if error}<p class="err">{error}</p>{/if}
</div>

<style>
  .onboard { max-width: 480px; margin: 40px auto; line-height: 1.5; display: flex; flex-direction: column; gap: 10px; align-items: flex-start; }
  .primary { font-weight: 600; }
  .err { color: #c00; }
</style>

<script lang="ts">
  import { api } from "./api";

  let { update, onDone }: { update: { version: string }, onDone: () => void } = $props();

  let busy = $state(false);
  let error = $state("");

  async function install() {
    busy = true;
    error = "";
    try {
      await api.downloadUpdate(update.version);
      onDone();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="banner">
  <span>New CloakBrowser binary available: <strong>{update.version}</strong></span>
  <button onclick={install} disabled={busy}>{busy ? "Downloading…" : "Update"}</button>
  <button onclick={onDone} disabled={busy}>Dismiss</button>
  {#if error}<span class="err">{error}</span>{/if}
</div>

<style>
  .banner {
    background: #243;
    color: #fff;
    padding: 8px 12px;
    border-radius: 6px;
    display: flex;
    gap: 12px;
    align-items: center;
    margin-bottom: 12px;
  }
  .err {
    color: #f88;
  }
</style>

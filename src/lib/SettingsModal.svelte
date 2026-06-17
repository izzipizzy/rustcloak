<script lang="ts">
  import { onMount } from "svelte";
  import { api } from "./api";

  let { onClose }: { onClose: () => void } = $props();

  let text = $state("");
  let clearing = $state(false);
  let clearedMb = $state<string | null>(null);
  onMount(async () => {
    const list = await api.getDefaultExtensions();
    text = list.join("\n");
  });

  async function save() {
    const sources = text
      .split(/\n+/)
      .map((s) => s.trim())
      .filter(Boolean);
    await api.setDefaultExtensions(sources);
    onClose();
  }

  async function clearCache() {
    clearing = true;
    clearedMb = null;
    try {
      const freed = await api.clearAllCaches();
      clearedMb = (freed / 1048576).toFixed(1);
    } finally {
      clearing = false;
    }
  }
</script>

<div class="backdrop" onclick={onClose} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog">
    <h2>Default extensions</h2>
    <p>
      One source per line (Web Store URL/id, .crx or .zip URL). These are auto-installed into every
      <strong>new</strong> profile you create.
    </p>
    <textarea
      bind:value={text}
      rows="8"
      placeholder="cjpalhdlnbpafiamejdnhcphjbkeiagm&#10;https://example.com/ext.crx"
    ></textarea>
    <div class="actions">
      <button onclick={onClose}>Cancel</button>
      <button onclick={save}>Save</button>
    </div>
    <hr />
    <h3>Cache</h3>
    <p>Delete browser cache for all stopped profiles. Cookies, history and logins are kept.</p>
    <button onclick={clearCache} disabled={clearing}>
      {clearing ? "Clearing…" : "Clear cache (keep cookies & history)"}
    </button>
    {#if clearedMb !== null}<p>Freed {clearedMb} MB.</p>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.5);
    display: flex;
    align-items: center;
    justify-content: center;
  }
  .modal {
    background: #ffffff;
    color: #1a1a1a;
    padding: 20px;
    border-radius: 8px;
    width: 420px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.3);
  }
  textarea {
    width: 100%;
    font-family: monospace;
    color: #1a1a1a;
    background: #fff;
    border: 1px solid #c8c8c8;
    border-radius: 4px;
    padding: 6px 8px;
    box-sizing: border-box;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
  hr {
    width: 100%;
    border: none;
    border-top: 1px solid #ddd;
  }
</style>

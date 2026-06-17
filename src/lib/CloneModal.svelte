<script lang="ts">
  import { api, type Profile } from "./api";

  let {
    source,
    onClose,
    onSaved,
  }: {
    source: Profile;
    onClose: () => void;
    onSaved: () => void;
  } = $props();

  let name = $state(`${source.name} (copy)`);
  let inheritProxy = $state(false);
  let busy = $state(false);
  let error = $state("");

  async function doClone() {
    busy = true;
    error = "";
    try {
      await api.clone(source.id, name, inheritProxy);
      onSaved();
      onClose();
    } catch (e) {
      error = String(e);
    } finally {
      busy = false;
    }
  }
</script>

<div class="backdrop" onclick={onClose} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog">
    <h2>Clone "{source.name}"</h2>
    <p>The cookies/storage are copied. A <strong>new fingerprint</strong> is generated.</p>
    <label>New name <input bind:value={name} /></label>
    <label><input type="checkbox" bind:checked={inheritProxy} /> Reuse the same proxy</label>
    {#if error}<p class="err">{error}</p>{/if}
    <div class="actions">
      <button onclick={onClose} disabled={busy}>Cancel</button>
      <button onclick={doClone} disabled={busy || !name.trim()}>{busy ? "Cloning…" : "Clone"}</button>
    </div>
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
    width: 360px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    box-shadow: 0 10px 40px rgba(0, 0, 0, 0.3);
  }
  .modal :global(input[type="text"]),
  .modal :global(input:not([type])) {
    color: #1a1a1a;
    background: #fff;
    border: 1px solid #c8c8c8;
    border-radius: 4px;
    padding: 6px 8px;
    font: inherit;
  }
  .err {
    color: #c00;
    font-size: 13px;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
</style>

<script lang="ts">
  import { onMount } from "svelte";
  import { api, type Profile } from "./api";

  let {
    onClone = (_p: Profile) => {},
    onEdit = (_p: Profile) => {},
  }: {
    onClone?: (p: Profile) => void;
    onEdit?: (p: Profile) => void;
  } = $props();

  let profiles = $state<Profile[]>([]);
  let sizes = $state<Record<string, number>>({});
  let search = $state("");

  const fmtMB = (b: number | undefined) =>
    b == null ? "…" : (b / 1048576).toFixed(1) + " MB";

  async function refresh() {
    profiles = await api.list();
    const entries = await Promise.all(
      profiles.map(async (p) => [p.id, await api.profileSize(p.id)] as const),
    );
    sizes = Object.fromEntries(entries);
  }
  onMount(refresh);

  let filtered = $derived(
    profiles.filter(
      (p) =>
        p.name.toLowerCase().includes(search.toLowerCase()) ||
        p.tags.some((t) => t.toLowerCase().includes(search.toLowerCase())),
    ),
  );

  async function launch(p: Profile) {
    await api.launch(p.id);
    await refresh();
  }
  async function stop(p: Profile) {
    await api.stop(p.id);
    await refresh();
  }
  async function checkProxy(p: Profile) {
    await api.checkProxy(p.id);
    await refresh();
  }
  async function remove(p: Profile) {
    if (confirm(`Delete profile "${p.name}"?`)) {
      await api.remove(p.id);
      await refresh();
    }
  }
  async function addExtensions(p: Profile) {
    const raw = prompt(
      "Extension sources (URLs, .crx/.zip, or Web Store ids) — separate by space or comma:",
    );
    if (!raw) return;
    const sources = raw
      .split(/[\s,]+/)
      .map((s) => s.trim())
      .filter(Boolean);
    if (sources.length === 0) return;
    const errors = await api.addExtensions(p.id, sources);
    alert(
      errors.length
        ? `Installed with errors:\n${errors.join("\n")}`
        : `Installed ${sources.length} extension(s)`,
    );
  }

  async function clearCache(p: Profile) {
    if (p.status.kind === "running") {
      alert("Stop the profile before clearing its cache.");
      return;
    }
    const freed = await api.clearProfileCache(p.id);
    await refresh();
    if (freed > 0) {
      // brief, non-modal feedback could go here; size column already updates.
    }
  }

  export function reload() {
    return refresh();
  }
</script>

<input class="search" placeholder="Search name or tag…" bind:value={search} />

<table>
  <thead>
    <tr><th>Name</th><th>Status</th><th>Size</th><th>Proxy</th><th>Tags</th><th>Actions</th></tr>
  </thead>
  <tbody>
    {#each filtered as p (p.id)}
      <tr>
        <td>{p.name}</td>
        <td>{p.status.kind === "running" ? `▶ :${p.status.cdp_port}` : "■ stopped"}</td>
        <td class="size">
          <span>{fmtMB(sizes[p.id])}</span>
          <button
            class="icon-btn"
            title="Clear cache (keeps cookies & history)"
            aria-label="Clear cache"
            onclick={() => clearCache(p)}
            disabled={p.status.kind === "running"}
          >
            <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <path d="m7 21-4.3-4.3c-1-1-1-2.5 0-3.4l9.6-9.6c1-1 2.5-1 3.4 0l5.6 5.6c1 1 1 2.5 0 3.4L13 21" />
              <path d="M22 21H7" />
              <path d="m5 11 9 9" />
            </svg>
          </button>
        </td>
        <td>
          {#if p.proxy}
            {p.proxy_status.kind === "ok"
              ? `${p.proxy_status.country} ${p.proxy_status.ip}`
              : p.proxy_status.kind === "dead"
                ? "✗ dead"
                : "?"}
          {:else}—{/if}
        </td>
        <td>{p.tags.join(", ")}</td>
        <td>
          {#if p.status.kind === "running"}
            <button onclick={() => stop(p)}>Stop</button>
          {:else}
            <button onclick={() => launch(p)}>Launch</button>
          {/if}
          <button onclick={() => checkProxy(p)} disabled={!p.proxy}>Check proxy</button>
          <button onclick={() => onEdit(p)}>Edit</button>
          <button onclick={() => onClone(p)}>Clone</button>
          <button onclick={() => addExtensions(p)}>+ Ext</button>
          <button onclick={() => remove(p)}>Delete</button>
        </td>
      </tr>
    {/each}
  </tbody>
</table>

<style>
  table {
    width: 100%;
    border-collapse: collapse;
  }
  th,
  td {
    text-align: left;
    padding: 6px 8px;
    border-bottom: 1px solid #2a2a2a;
  }
  .search {
    margin-bottom: 12px;
    padding: 6px;
    width: 280px;
  }
  button {
    margin-right: 4px;
  }
  .size {
    white-space: nowrap;
  }
  .icon-btn {
    margin-left: 6px;
    padding: 2px;
    border: none;
    background: transparent;
    color: #888;
    cursor: pointer;
    vertical-align: middle;
    line-height: 0;
  }
  .icon-btn:hover:not(:disabled) {
    color: #1a73e8;
  }
  .icon-btn:disabled {
    opacity: 0.3;
    cursor: default;
  }
</style>

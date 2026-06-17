<script lang="ts">
  import { api, type Profile, type NewProfile, type OsProfile, type GeoMode } from "./api";
  import { LOCALES, TIMEZONES } from "./geodata";

  let {
    editing = null,
    onClose,
    onSaved,
  }: {
    editing?: Profile | null;
    onClose: () => void;
    onSaved: () => void;
  } = $props();

  let name = $state(editing?.name ?? "");
  let os_profile = $state<OsProfile>(editing?.os_profile ?? "mac");
  let proxy = $state(editing?.proxy ?? "");
  let tags = $state((editing?.tags ?? []).join(", "));
  let group = $state(editing?.group ?? "");
  let notes = $state(editing?.notes ?? "");
  let language_mode = $state<GeoMode>(editing?.language_mode ?? "auto");
  let language = $state(editing?.language ?? "");
  let timezone_mode = $state<GeoMode>(editing?.timezone_mode ?? "auto");
  let timezone = $state(editing?.timezone ?? "");

  async function save() {
    const tagList = tags
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean);
    if (editing) {
      const updated: Profile = {
        ...editing,
        name,
        os_profile,
        proxy: proxy || null,
        tags: tagList,
        group: group || null,
        notes,
        language_mode,
        language: language_mode === "manual" ? (language.trim() || null) : null,
        timezone_mode,
        timezone: timezone_mode === "manual" ? (timezone.trim() || null) : null,
      };
      await api.update(updated);
    } else {
      const np: NewProfile = {
        name,
        os_profile,
        proxy: proxy || null,
        tags: tagList,
        group: group || null,
        notes,
        language_mode,
        language: language_mode === "manual" ? (language.trim() || null) : null,
        timezone_mode,
        timezone: timezone_mode === "manual" ? (timezone.trim() || null) : null,
      };
      await api.create(np);
    }
    onSaved();
    onClose();
  }
</script>

<div class="backdrop" onclick={onClose} role="presentation">
  <div class="modal" onclick={(e) => e.stopPropagation()} role="dialog">
    <h2>{editing ? "Edit profile" : "New profile"}</h2>
    <label>Name <input bind:value={name} /></label>
    <label
      >OS fingerprint
      <select bind:value={os_profile}>
        <option value="mac">macOS</option>
        <option value="windows">Windows</option>
      </select>
    </label>
    <label>Proxy <input bind:value={proxy} placeholder="http://user:pass@host:port" /></label>
    <label
      >Language
      <select bind:value={language_mode}>
        <option value="auto">Auto (by proxy IP)</option>
        <option value="manual">Manual</option>
      </select>
    </label>
    {#if language_mode === "manual"}
      <label>Locale
        <input list="locale-options" bind:value={language} placeholder="Search e.g. Finnish or fi-FI" autocomplete="off" />
      </label>
      <datalist id="locale-options">
        {#each LOCALES as l}
          <option value={l.code}>{l.name}</option>
        {/each}
      </datalist>
    {/if}
    <label
      >Timezone
      <select bind:value={timezone_mode}>
        <option value="auto">Auto (by proxy IP)</option>
        <option value="manual">Manual</option>
      </select>
    </label>
    {#if timezone_mode === "manual"}
      <label>IANA timezone
        <input list="timezone-options" bind:value={timezone} placeholder="Search e.g. Helsinki" autocomplete="off" />
      </label>
      <datalist id="timezone-options">
        {#each TIMEZONES as tz}
          <option value={tz}>{tz}</option>
        {/each}
      </datalist>
    {/if}
    <label>Tags <input bind:value={tags} placeholder="ads, social" /></label>
    <label>Group <input bind:value={group} /></label>
    <label>Notes <textarea bind:value={notes}></textarea></label>
    <div class="actions">
      <button onclick={onClose}>Cancel</button>
      <button onclick={save} disabled={!name.trim()}>Save</button>
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
  .modal :global(input),
  .modal :global(select),
  .modal :global(textarea) {
    color: #1a1a1a;
    background: #fff;
    border: 1px solid #c8c8c8;
    border-radius: 4px;
    padding: 6px 8px;
    font: inherit;
  }
  label {
    display: flex;
    flex-direction: column;
    font-size: 13px;
    gap: 4px;
    color: #333;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin-top: 12px;
  }
</style>

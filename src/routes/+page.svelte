<script lang="ts">
  import { onMount } from "svelte";
  import ProfileList from "$lib/ProfileList.svelte";
  import ProfileModal from "$lib/ProfileModal.svelte";
  import CloneModal from "$lib/CloneModal.svelte";
  import Onboarding from "$lib/Onboarding.svelte";
  import SettingsModal from "$lib/SettingsModal.svelte";
  import UpdateBanner from "$lib/UpdateBanner.svelte";
  import { api, type Profile } from "$lib/api";

  let list = $state<ReturnType<typeof ProfileList> | undefined>();
  let showModal = $state(false);
  let editing = $state<Profile | null>(null);
  let cloneSource = $state<Profile | null>(null);
  let showSettings = $state(false);
  let engineReady = $state(false);
  let update: { version: string } | null = $state(null);

  onMount(async () => {
    engineReady = await api.engineConfigured();
    update = await api.checkForUpdate(false);
  });

  function newProfile() {
    editing = null;
    showModal = true;
  }
  function onEdit(p: Profile) {
    editing = p;
    showModal = true;
  }
  function onClone(p: Profile) {
    cloneSource = p;
  }
  function afterSave() {
    list?.reload();
  }
</script>

<main>
  {#if !engineReady}
    <Onboarding onDone={() => (engineReady = true)} />
  {:else}
    {#if update}
      <UpdateBanner {update} onDone={() => (update = null)} />
    {/if}
    <header>
      <h1>rustcloak</h1>
      <div>
        <button onclick={() => (showSettings = true)}>Default extensions</button>
        <button onclick={newProfile}>+ New profile</button>
      </div>
    </header>
    <ProfileList bind:this={list} {onClone} {onEdit} />
    {#if showModal}
      <ProfileModal {editing} onClose={() => (showModal = false)} onSaved={afterSave} />
    {/if}
    {#if cloneSource}
      <CloneModal source={cloneSource} onClose={() => (cloneSource = null)} onSaved={afterSave} />
    {/if}
    {#if showSettings}
      <SettingsModal onClose={() => (showSettings = false)} />
    {/if}
  {/if}
</main>

<style>
  main {
    padding: 16px;
    font-family: system-ui, sans-serif;
  }
  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
</style>

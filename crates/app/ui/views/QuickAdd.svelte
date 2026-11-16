<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, KINDS, type Kind } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { ui, toast, changed } from "../lib/state.svelte";

  let text = $state("");
  let kind = $state<Kind | null>(null);
  let preview = $state<string | null>(null);
  let problem = $state<string | null>(null);
  let busy = $state(false);
  let input: HTMLInputElement;

  onMount(() => input.focus());

  $effect(() => {
    const value = text.trim();
    if (!value) {
      preview = null;
      problem = null;
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const p = await api.previewQuickAdd(value);
        preview = t("quick.means", { title: p.title, label: p.due_label });
        problem = null;
      } catch (e) {
        preview = null;
        problem = errorText(e);
      }
    }, 120);
    return () => clearTimeout(timer);
  });

  async function submit(e: Event) {
    e.preventDefault();
    if (!text.trim() || busy) return;
    busy = true;
    try {
      const item = await api.quickAdd(text.trim(), kind);
      toast(t("quick.added", { title: item.title, label: item.due_label }));
      changed();
      ui.quickAdd = false;
    } catch (err) {
      problem = errorText(err);
    } finally {
      busy = false;
    }
  }
</script>

<div class="scrim" role="presentation" onclick={() => (ui.quickAdd = false)}></div>
<form class="box" onsubmit={submit}>
  <input bind:this={input} bind:value={text} placeholder={t("quick.placeholder")} aria-label={t("nav.quickAdd")} autocomplete="off" spellcheck="false" />
  <div class="line small">
    {#if preview}
      <span class="ok">→ {preview}</span>
    {:else if problem && text.trim().split(/\s+/).length > 1}
      <span class="muted">{problem.split(". It is now")[0]}.</span>
    {:else}
      <span class="muted">{t("quick.hint")}</span>
    {/if}
  </div>
  <div class="kinds">
    {#each KINDS as k (k)}
      <button type="button" class="chip" class:on={kind === k} onclick={() => (kind = kind === k ? null : k)}>{t(`kind.${k}`)}</button>
    {/each}
  </div>
</form>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(20, 18, 15, 0.22);
    z-index: 30;
  }
  .box {
    position: fixed;
    top: 16vh;
    left: 50%;
    transform: translateX(-50%);
    width: min(620px, calc(100vw - 32px));
    background: var(--surface);
    border-radius: 14px;
    box-shadow: var(--shadow);
    border: 1px solid var(--line);
    padding: 14px;
    z-index: 31;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
  input {
    font-size: 19px;
    padding: 12px 14px !important;
  }
  .line {
    min-height: 20px;
    padding: 0 4px;
  }
  .ok {
    color: var(--ink-2);
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    font-size: 13px;
    padding: 3px 10px;
    border-radius: 99px;
    color: var(--muted);
  }
  .chip.on {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }
</style>

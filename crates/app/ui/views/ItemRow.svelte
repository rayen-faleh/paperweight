<script lang="ts">
  import { api, localTime, errorText, type Item } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { ui, toast, changed } from "../lib/state.svelte";

  let { item, showDay = false }: { item: Item; showDay?: boolean } = $props();
  let busy = $state(false);

  async function complete(e: Event) {
    e.stopPropagation();
    busy = true;
    try {
      const r = await api.completeItem(item.id);
      toast(r.next ? t("detail.nextCreated", { label: r.next.due_label }) : t("detail.done"));
      changed();
    } catch (err) {
      toast(t("common.error", { message: errorText(err) }));
    } finally {
      busy = false;
    }
  }
</script>

<div
  class="row"
  class:overdue={item.overdue}
  class:done={item.status === "done"}
  role="button"
  tabindex="0"
  onclick={() => (ui.selected = item.id)}
  onkeydown={(e) => e.key === "Enter" && (ui.selected = item.id)}
>
  <div class="when tnum">
    {#if showDay}<span class="day">{item.due_label.replace(/^overdue \((.*)\)$/, "$1").replace(/ \d\d:\d\d$/, "")}</span>{/if}
    {#if item.all_day}<span class="muted">{t("row.allDay")}</span>{:else}{localTime(item.due_local)}{/if}
  </div>
  <div class="body">
    <div class="title">{item.title}</div>
    <div class="meta small">
      <span>{t(`kind.${item.kind}`)}</span>
      {#if item.requirements.length}<span>· {t("row.needs", { done: item.requirements_done, total: item.requirements.length })}</span>{/if}
      {#if item.steps.length}<span>· {t("row.steps", { done: item.steps_done, total: item.steps.length })}</span>{/if}
      {#if item.recurrence.freq !== "none"}<span>· {t("row.repeats")}</span>{/if}
      {#if item.snoozed}<span class="snz">· {t("row.snoozed")}</span>{/if}
      {#if item.overdue}<span class="late">· {item.due_label}</span>{/if}
    </div>
  </div>
  {#if item.status === "open"}
    <button class="tick" title={t("row.complete")} aria-label={t("row.complete")} disabled={busy} onclick={complete}></button>
  {/if}
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 76px 1fr auto;
    gap: 14px;
    align-items: center;
    padding: 12px 14px;
    border-radius: var(--radius);
    cursor: pointer;
    outline: none;
  }
  .row:hover,
  .row:focus-visible {
    background: var(--surface);
    box-shadow: var(--shadow);
  }
  .when {
    font-size: 14px;
    color: var(--ink-2);
    display: flex;
    flex-direction: column;
    line-height: 1.3;
  }
  .day {
    font-size: 12px;
    color: var(--muted);
  }
  .title {
    font-weight: 500;
    overflow-wrap: anywhere;
  }
  .meta {
    color: var(--muted);
    display: flex;
    flex-wrap: wrap;
    gap: 0 5px;
  }
  .overdue .when {
    color: var(--accent);
    font-weight: 600;
  }
  .late {
    color: var(--accent);
  }
  .snz {
    font-style: italic;
  }
  .done .title {
    text-decoration: line-through;
    color: var(--muted);
  }
  .tick {
    width: 22px;
    height: 22px;
    padding: 0;
    border-radius: 50%;
    border: 1.5px solid var(--line);
    background: transparent;
    opacity: 0.7;
  }
  .row:hover .tick {
    border-color: var(--muted);
    opacity: 1;
  }
  .tick:hover {
    background: var(--accent-soft);
    border-color: var(--accent) !important;
  }
</style>

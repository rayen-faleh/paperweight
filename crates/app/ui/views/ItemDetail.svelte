<script lang="ts">
  import { api, errorText, localTime, KINDS, type Changes, type Freq, type Item } from "../lib/api";
  import { i18n, t } from "../lib/i18n.svelte";
  import { ui, toast, changed, dayName, shortDate } from "../lib/state.svelte";

  let { id }: { id: string } = $props();

  let item = $state<Item | null>(null);
  let error = $state<string | null>(null);
  // Editable copies of the text fields; saved on blur.
  let title = $state("");
  let due = $state("");
  let duePreview = $state<string | null>(null);
  let dueError = $state<string | null>(null);
  let notes = $state("");
  let source = $state("");
  let newStep = $state("");
  let newReq = $state("");
  let snoozeOpen = $state(false);
  let snoozeCustom = $state("");

  function fill(it: Item) {
    item = it;
    title = it.title;
    due = "";
    notes = it.notes;
    source = it.source ?? "";
  }

  async function load() {
    try {
      fill(await api.getItem(id));
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }

  // Reload when another process changes the data, unless the person is typing.
  $effect(() => {
    void ui.version;
    void id;
    const active = document.activeElement;
    if (item && active && active.closest?.(".panel") && (active instanceof HTMLInputElement || active instanceof HTMLTextAreaElement)) {
      return;
    }
    load();
  });

  async function save(changes: Changes, message?: string) {
    if (!item) return;
    try {
      fill(await api.updateItem(item.id, changes));
      changed();
      if (message) toast(message);
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }

  $effect(() => {
    const text = due.trim();
    if (!text) {
      duePreview = null;
      dueError = null;
      return;
    }
    const timer = setTimeout(async () => {
      try {
        const p = await api.previewDue(text);
        duePreview = p.due_label;
        dueError = null;
      } catch (e) {
        duePreview = null;
        dueError = errorText(e);
      }
    }, 150);
    return () => clearTimeout(timer);
  });

  async function saveDue() {
    if (!due.trim() || dueError) return;
    await save({ due: due.trim() });
  }

  async function complete() {
    if (!item) return;
    try {
      const r = await api.completeItem(item.id);
      toast(r.next ? t("detail.nextCreated", { label: r.next.due_label }) : t("detail.done"));
      changed();
      ui.selected = null;
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }

  async function snooze(until: string) {
    if (!item) return;
    try {
      fill(await api.snoozeItem(item.id, until));
      snoozeOpen = false;
      snoozeCustom = "";
      changed();
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }

  async function remove() {
    if (!item) return;
    try {
      await api.deleteItem(item.id);
      toast(t("detail.deleted"));
      changed();
      ui.selected = null;
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }

  async function restore() {
    if (!item) return;
    try {
      fill(await api.restoreItem(item.id));
      changed();
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }

  function when(iso: string): string {
    return new Date(iso).toLocaleString(i18n.lang === "ja" ? "ja-JP" : "en-GB", {
      weekday: "short", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit",
    });
  }

  function dueText(it: Item): string {
    const day = dayName(it.due_day, i18n.lang, true);
    return it.all_day ? `${day} · ${t("row.allDay")}` : `${day} · ${localTime(it.due_local)}`;
  }
</script>

<div class="scrim" role="presentation" onclick={() => (ui.selected = null)}></div>
<aside class="panel" aria-label={item?.title ?? ""}>
  {#if error}
    <p class="muted">{t("common.error", { message: error })}</p>
    <button onclick={() => (ui.selected = null)}>{t("detail.close")}</button>
  {:else if item}
    <div class="top">
      <span class="label">{t(`kind.${item.kind}`)}{#if item.status !== "open"} · {t(`status.${item.status === "done" ? "done" : "open"}`)}{/if}</span>
      <button class="ghost close" onclick={() => (ui.selected = null)} aria-label={t("detail.close")}>✕</button>
    </div>

    <input
      class="title-input"
      bind:value={title}
      aria-label={t("detail.title")}
      onblur={() => title.trim() && title !== item?.title && save({ title })}
      onkeydown={(e) => e.key === "Enter" && (e.currentTarget as HTMLInputElement).blur()}
    />

    <div class="due" class:late={item.overdue}>
      <div>
        <div class="due-main">{dueText(item)}</div>
        {#if /^(today|tonight|tomorrow|overdue)/.test(item.due_label)}<div class="muted small">{item.due_label}</div>{/if}
      </div>
    </div>

    {#if item.status === "archived"}
      <p class="muted">{t("detail.deleted")}</p>
      <button class="primary" onclick={restore}>{t("detail.restore")}</button>
    {:else}
      <div class="actions">
        {#if item.status === "open"}
          <button class="primary" onclick={complete}>{t("detail.complete")}</button>
        {:else}
          <button onclick={() => save({ status: "open" })}>{t("detail.reopen")}</button>
        {/if}
        {#if item.snoozed}
          <button onclick={() => save({ clear_snooze: true })}>{t("detail.unsnooze")}</button>
        {:else if item.status === "open"}
          <button onclick={() => (snoozeOpen = !snoozeOpen)} aria-expanded={snoozeOpen}>{t("detail.snooze")}</button>
        {/if}
        <button class="ghost danger" onclick={remove}>{t("detail.delete")}</button>
      </div>
      {#if item.snoozed && item.snoozed_until}
        <p class="muted small">{t("detail.snoozedUntil", { time: item.snoozed_until_label ?? when(item.snoozed_until) })}</p>
      {/if}
      {#if snoozeOpen}
        <div class="snooze">
          <div class="chips">
            <button onclick={() => snooze("tonight 22:00")}>{t("detail.snooze.tonight")}</button>
            <button onclick={() => snooze("tomorrow 12:00")}>{t("detail.snooze.tomorrow")}</button>
            <button onclick={() => snooze("in 7 days 12:00")}>{t("detail.snooze.week")}</button>
          </div>
          <form onsubmit={(e) => { e.preventDefault(); if (snoozeCustom.trim()) snooze(snoozeCustom.trim()); }}>
            <input placeholder={t("detail.snooze.custom")} bind:value={snoozeCustom} />
          </form>
          <p class="muted small">{t("detail.snoozeNote")}</p>
        </div>
      {/if}
    {/if}

    {#if item.requirements.length || item.status === "open"}
      <section>
        <h3 class="label">{t("detail.needed")} {#if item.requirements.length}<span class="count">{item.requirements_done}/{item.requirements.length}</span>{/if}</h3>
        <ul class="checks">
          {#each item.requirements as r (r.id)}
            <li>
              <label>
                <input class="check" type="checkbox" checked={r.done} onchange={(e) => save({ set_requirement: [r.id, (e.currentTarget as HTMLInputElement).checked] })} />
                <span class:struck={r.done}>{r.text}</span>
              </label>
              <button class="ghost x" aria-label={t("detail.remove")} onclick={() => save({ remove_requirement: r.id })}>✕</button>
            </li>
          {/each}
        </ul>
        <form onsubmit={(e) => { e.preventDefault(); if (newReq.trim()) { save({ add_requirement: newReq.trim() }); newReq = ""; } }}>
          <input class="add-line" placeholder={t("detail.addNeeded")} bind:value={newReq} />
        </form>
      </section>
    {/if}

    {#if item.steps.length || item.status === "open"}
      <section>
        <h3 class="label">{t("detail.steps")} {#if item.steps.length}<span class="count">{item.steps_done}/{item.steps.length}</span>{/if}</h3>
        <ol class="checks">
          {#each item.steps as s (s.id)}
            <li>
              <label>
                <input class="check" type="checkbox" checked={s.done} onchange={(e) => save({ set_step: [s.id, (e.currentTarget as HTMLInputElement).checked] })} />
                <span class:struck={s.done}>{s.title}</span>
              </label>
              <button class="ghost x" aria-label={t("detail.remove")} onclick={() => save({ remove_step: s.id })}>✕</button>
            </li>
          {/each}
        </ol>
        <form onsubmit={(e) => { e.preventDefault(); if (newStep.trim()) { save({ add_step: newStep.trim() }); newStep = ""; } }}>
          <input class="add-line" placeholder={t("detail.addStep")} bind:value={newStep} />
        </form>
      </section>
    {/if}

    <section class="fields">
      <label class="field">
        <span class="label">{t("detail.due")}</span>
        <input
          placeholder={t("detail.duePlaceholder")}
          bind:value={due}
          onblur={saveDue}
          onkeydown={(e) => e.key === "Enter" && saveDue()}
        />
        {#if duePreview}<span class="small hint">{t("detail.dueMeans", { label: duePreview })}</span>{/if}
        {#if dueError}<span class="small err">{dueError}</span>{/if}
      </label>
      <div class="two">
        <label class="field">
          <span class="label">{t("detail.kind")}</span>
          <select value={item.kind} onchange={(e) => save({ kind: (e.currentTarget as HTMLSelectElement).value as Item["kind"] })}>
            {#each KINDS as k (k)}<option value={k}>{t(`kind.${k}`)}</option>{/each}
          </select>
        </label>
        <label class="field">
          <span class="label">{t("detail.estimate")}</span>
          <div class="inline">
            <input type="number" min="0" max="10080" step="15" value={item.estimated_minutes}
              onchange={(e) => save({ estimated_minutes: Math.max(0, Number((e.currentTarget as HTMLInputElement).value) || 0) })} />
            <span class="muted small">{t("detail.minutes")}</span>
          </div>
        </label>
      </div>
      <label class="field">
        <span class="label">{t("detail.repeat")}</span>
        <div class="inline">
          <select value={item.recurrence.freq}
            onchange={(e) => save({ recurrence: { freq: (e.currentTarget as HTMLSelectElement).value as Freq, interval: item?.recurrence.interval ?? 1 } })}>
            {#each ["none", "daily", "weekly", "monthly", "yearly"] as f (f)}<option value={f}>{t(`detail.freq.${f as Freq}`)}</option>{/each}
          </select>
          {#if item.recurrence.freq !== "none"}
            <span class="muted small">{t("detail.every")}</span>
            <input class="narrow" type="number" min="1" max="366" value={item.recurrence.interval}
              onchange={(e) => save({ recurrence: { freq: item!.recurrence.freq, interval: Math.max(1, Number((e.currentTarget as HTMLInputElement).value) || 1) } })} />
          {/if}
        </div>
      </label>
      <label class="field">
        <span class="label">{t("detail.source")}</span>
        <input placeholder={t("detail.sourcePlaceholder")} bind:value={source} onblur={() => source !== (item?.source ?? "") && save({ source })} />
      </label>
      <label class="field">
        <span class="label">{t("detail.notes")}</span>
        <textarea bind:value={notes} onblur={() => notes !== item?.notes && save({ notes })}></textarea>
      </label>
    </section>

    <p class="muted small foot">
      {t("detail.id", { id: item.id })}<br />
      {t("detail.created", { date: shortDate(item.created_at, i18n.lang) })}
    </p>
  {:else}
    <p class="muted">{t("common.loading")}</p>
  {/if}
</aside>

<style>
  .scrim {
    position: fixed;
    inset: 0;
    background: rgba(20, 18, 15, 0.18);
    z-index: 20;
  }
  .panel {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(520px, 100vw);
    background: var(--surface);
    box-shadow: var(--shadow);
    border-left: 1px solid var(--line);
    z-index: 21;
    overflow-y: auto;
    padding: 22px 28px 40px;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .close {
    padding: 4px 9px;
  }
  .title-input {
    font-size: 22px;
    font-weight: 600;
    border-color: transparent !important;
    padding: 4px 6px;
    margin: -6px -6px 0;
    width: calc(100% + 12px) !important;
    background: transparent !important;
  }
  .title-input:focus {
    border-color: var(--accent) !important;
  }
  .due-main {
    font-size: 17px;
    font-weight: 500;
  }
  .late .due-main {
    color: var(--accent);
  }
  .actions,
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }
  .snooze {
    background: var(--sunken);
    border-radius: var(--radius);
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .snooze p {
    margin: 0;
  }
  section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .count {
    margin-left: 6px;
    font-weight: 500;
    letter-spacing: 0;
  }
  .checks {
    list-style: none;
    margin: 0;
    padding: 0;
  }
  .checks li {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    padding: 5px 0;
  }
  .checks label {
    display: flex;
    gap: 10px;
    align-items: flex-start;
    cursor: pointer;
  }
  .checks .check {
    margin-top: 2px;
  }
  .struck {
    text-decoration: line-through;
    color: var(--muted);
  }
  .x {
    padding: 2px 7px;
    font-size: 12px;
    opacity: 0;
  }
  li:hover .x,
  .x:focus-visible {
    opacity: 0.7;
  }
  .add-line {
    border-style: dashed !important;
    background: transparent !important;
  }
  .fields {
    gap: 14px;
    padding-top: 6px;
    border-top: 1px solid var(--line);
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .two {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
  }
  .inline {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .narrow {
    width: 80px !important;
  }
  .hint {
    color: var(--ink-2);
  }
  .err {
    color: var(--accent);
  }
  .foot {
    margin-top: 8px;
  }
</style>

<script lang="ts">
  import { api, errorText, type Item } from "../lib/api";
  import { i18n, t } from "../lib/i18n.svelte";
  import { ui, toast, changed, shortDate } from "../lib/state.svelte";

  type Tab = "done" | "deleted";
  const TAB_KEY = "paperweight.archiveTab";

  function savedTab(): Tab {
    try {
      return localStorage.getItem(TAB_KEY) === "deleted" ? "deleted" : "done";
    } catch {
      return "done";
    }
  }

  let tab = $state<Tab>(savedTab());
  let items = $state<Item[]>([]);
  let loaded = $state(false);
  let error = $state<string | null>(null);

  function setTab(next: Tab) {
    if (next === tab) return;
    tab = next;
    loaded = false;
    try {
      localStorage.setItem(TAB_KEY, next);
    } catch {
      /* per-viewer convenience only */
    }
  }

  $effect(() => {
    void ui.version;
    const which = tab;
    api.listItems(which === "done" ? "done" : "archived", "", "").then(
      (v) => {
        if (which !== tab) return;
        // Done: most recently completed first. Deleted: most recently deleted first.
        const stamp = (i: Item) => (which === "done" ? i.completed_at : i.archived_at) ?? i.updated_at;
        items = v.slice().sort((a, b) => stamp(b).localeCompare(stamp(a)));
        error = null;
        loaded = true;
      },
      (e) => ((error = errorText(e)), (loaded = true)),
    );
  });

  async function restore(item: Item) {
    try {
      await api.restoreItem(item.id);
      toast(t("archive.restored", { title: item.title }));
      changed();
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }

  async function reopen(item: Item) {
    try {
      await api.updateItem(item.id, { status: "open" });
      toast(t("archive.reopened", { title: item.title }));
      changed();
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }

  const due = (item: Item) => item.due_label.replace(/^overdue \((.*)\)$/, "$1");
</script>

<section class="page">
  <header>
    <h1>{t("archive.title")}</h1>
    <div class="seg" role="radiogroup" aria-label={t("archive.tabs")}>
      <span class="pill" class:right={tab === "deleted"} aria-hidden="true"></span>
      <button role="radio" aria-checked={tab === "done"} class:on={tab === "done"} onclick={() => setTab("done")}>
        {t("archive.tab.done")}
      </button>
      <button role="radio" aria-checked={tab === "deleted"} class:on={tab === "deleted"} onclick={() => setTab("deleted")}>
        {t("archive.tab.deleted")}
      </button>
    </div>
  </header>
  <p class="muted">{tab === "done" ? t("archive.doneHint") : t("archive.hint")}</p>

  {#if error}
    <p class="muted">{t("common.error", { message: error })}</p>
  {:else if loaded && items.length === 0}
    <div class="empty">{tab === "done" ? t("archive.doneEmpty") : t("archive.empty")}</div>
  {:else if tab === "done"}
    <ul>
      {#each items as item (item.id)}
        <li>
          <button class="open" onclick={() => (ui.selected = item.id)}>
            <svg class="check" viewBox="0 0 16 16" aria-hidden="true"><circle cx="8" cy="8" r="7" /><path d="M4.8 8.2l2.2 2.2 4.2-4.6" /></svg>
            <span>
              <span class="title done">{item.title}</span>
              <span class="muted small meta">
                {t(`kind.${item.kind}`)} · {due(item)}
                {#if item.completed_at} · {t("archive.doneOn", { date: shortDate(item.completed_at, i18n.lang) })}{/if}
              </span>
            </span>
          </button>
          <button onclick={() => reopen(item)}>{t("archive.reopen")}</button>
        </li>
      {/each}
    </ul>
  {:else}
    <ul>
      {#each items as item (item.id)}
        <li>
          <div>
            <div class="title">{item.title}</div>
            <div class="muted small">
              {t(`kind.${item.kind}`)} · {due(item)}
              {#if item.archived_at} · {t("archive.deletedOn", { date: shortDate(item.archived_at, i18n.lang) })}{/if}
            </div>
          </div>
          <button onclick={() => restore(item)}>{t("archive.restore")}</button>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .page {
    max-width: 760px;
    margin: 0 auto;
  }
  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    flex-wrap: wrap;
  }
  h1 {
    font-size: 26px;
  }
  .page > p {
    margin: 6px 0 22px;
  }
  .seg {
    position: relative;
    display: grid;
    grid-template-columns: 1fr 1fr;
    background: var(--sunken);
    border-radius: 10px;
    padding: 3px;
  }
  .pill {
    position: absolute;
    top: 3px;
    bottom: 3px;
    left: 3px;
    width: calc(50% - 3px);
    background: var(--surface);
    border-radius: 8px;
    box-shadow: 0 1px 3px rgba(31, 29, 26, 0.12);
    transition: transform 0.42s cubic-bezier(0.22, 1.12, 0.36, 1);
  }
  .pill.right {
    transform: translateX(100%);
  }
  .seg button {
    position: relative;
    z-index: 1;
    border: 0;
    background: transparent;
    padding: 7px 18px;
    font-size: 13.5px;
    color: var(--muted);
    white-space: nowrap;
  }
  .seg button.on {
    color: var(--ink);
    font-weight: 600;
  }
  ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  li {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 12px 0;
    border-bottom: 1px solid var(--line);
  }
  .open {
    display: flex;
    align-items: flex-start;
    gap: 12px;
    flex: 1;
    min-width: 0;
    border: 0;
    background: transparent;
    padding: 0;
    text-align: left;
    font: inherit;
    color: inherit;
    cursor: pointer;
  }
  .open:hover .title {
    color: var(--ink);
  }
  .check {
    flex: none;
    width: 16px;
    height: 16px;
    margin-top: 2px;
    fill: none;
    stroke: var(--muted);
    stroke-width: 1.4;
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .title {
    display: block;
    color: var(--ink-2);
  }
  .title.done {
    text-decoration: line-through;
    text-decoration-color: var(--line);
  }
  .meta {
    display: block;
  }
  .empty {
    padding: 36px;
    text-align: center;
    border: 1px dashed var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
  @media (prefers-reduced-motion: reduce) {
    .pill {
      transition: none;
    }
  }
</style>

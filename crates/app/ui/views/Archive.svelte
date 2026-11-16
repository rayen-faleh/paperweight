<script lang="ts">
  import { api, errorText, type Item } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { ui, toast, changed } from "../lib/state.svelte";

  let items = $state<Item[]>([]);
  let error = $state<string | null>(null);

  $effect(() => {
    void ui.version;
    api.listItems("archived", "", "").then(
      (v) => ((items = v), (error = null)),
      (e) => (error = errorText(e)),
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
</script>

<section class="page">
  <h1>{t("archive.title")}</h1>
  <p class="muted">{t("archive.hint")}</p>
  {#if error}
    <p class="muted">{t("common.error", { message: error })}</p>
  {:else if items.length === 0}
    <div class="empty">{t("archive.empty")}</div>
  {:else}
    <ul>
      {#each items as item (item.id)}
        <li>
          <div>
            <div class="title">{item.title}</div>
            <div class="muted small">
              {t(`kind.${item.kind}`)} · {item.due_label.replace(/^overdue \((.*)\)$/, "$1")}
              {#if item.archived_at} · {t("archive.deletedOn", { date: new Date(item.archived_at).toLocaleDateString() })}{/if}
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
  h1 {
    font-size: 26px;
  }
  .page > p {
    margin: 6px 0 22px;
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
  .title {
    color: var(--ink-2);
  }
  .empty {
    padding: 36px;
    text-align: center;
    border: 1px dashed var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }
</style>

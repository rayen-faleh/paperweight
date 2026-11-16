<script lang="ts">
  import { api, errorText, KINDS, type Item } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { ui } from "../lib/state.svelte";
  import ItemRow from "./ItemRow.svelte";

  let status = $state("open");
  let kind = $state("");
  let query = $state("");
  let items = $state<Item[]>([]);
  let error = $state<string | null>(null);

  $effect(() => {
    void ui.version;
    const [s, k, q] = [status, kind, query];
    const timer = setTimeout(async () => {
      try {
        items = await api.listItems(s, k, q);
        error = null;
      } catch (e) {
        error = errorText(e);
      }
    }, q ? 120 : 0);
    return () => clearTimeout(timer);
  });
</script>

<section class="page">
  <h1>{t("nav.all")}</h1>
  <div class="filters">
    <input class="search" type="text" placeholder={t("all.search")} bind:value={query} />
    <select bind:value={status} aria-label="Status">
      <option value="open">{t("status.open")}</option>
      <option value="done">{t("status.done")}</option>
      <option value="all">{t("status.all")}</option>
    </select>
    <select bind:value={kind} aria-label={t("detail.kind")}>
      <option value="">{t("kind.all")}</option>
      {#each KINDS as k (k)}<option value={k}>{t(`kind.${k}`)}</option>{/each}
    </select>
  </div>
  {#if error}
    <p class="muted">{t("common.error", { message: error })}</p>
  {:else if items.length === 0}
    <p class="muted">{t("all.empty")}</p>
  {:else}
    <p class="muted small count">{t("all.count", { n: items.length })}</p>
    <div class="list">
      {#each items as item (item.id)}<ItemRow {item} showDay />{/each}
    </div>
  {/if}
</section>

<style>
  .page {
    max-width: 760px;
    margin: 0 auto;
  }
  h1 {
    font-size: 26px;
    margin-bottom: 20px;
  }
  .filters {
    display: grid;
    grid-template-columns: 1fr 150px 150px;
    gap: 8px;
    margin-bottom: 18px;
  }
  .count {
    margin: 0 0 6px;
  }
  .list {
    margin: 0 -14px;
  }
  @media (max-width: 720px) {
    .filters {
      grid-template-columns: 1fr 1fr;
    }
    .search {
      grid-column: 1 / -1;
    }
  }
</style>

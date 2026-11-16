<script lang="ts">
  import { onMount } from "svelte";
  import { api, errorText, type Brief, type Item, type WeekPlan } from "../lib/api";
  import { i18n, t } from "../lib/i18n.svelte";
  import { ui, dayName, addDays, prettyShortcut } from "../lib/state.svelte";
  import ItemRow from "./ItemRow.svelte";

  let brief = $state<Brief | null>(null);
  let plan = $state<WeekPlan | null>(null);
  let error = $state<string | null>(null);

  async function load() {
    try {
      [brief, plan] = await Promise.all([api.brief(), api.planWeek()]);
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }

  $effect(() => {
    void ui.version;
    load();
  });

  onMount(() => {
    // Time passes: "today", overdue and labels move even when nothing changes.
    const timer = setInterval(load, 30_000);
    return () => clearInterval(timer);
  });

  // Upcoming items grouped by their logical day.
  const groups = $derived.by(() => {
    if (!brief) return [] as { day: string; items: Item[] }[];
    const map = new Map<string, Item[]>();
    for (const it of brief.next_7_days) {
      if (!map.has(it.due_day)) map.set(it.due_day, []);
      map.get(it.due_day)!.push(it);
    }
    return [...map.entries()].map(([day, items]) => ({ day, items }));
  });

  function heading(day: string): string {
    if (brief && day === addDays(brief.today, 1)) return t("today.tomorrow");
    return dayName(day, i18n.lang, true);
  }

  const total = $derived(brief ? brief.overdue.length + brief.due_today.length + brief.next_7_days.length : 0);
  const sessions = $derived(plan ? plan.sessions.slice(0, 5) : []);
</script>

<section class="page">
  {#if error}
    <p class="muted">{t("common.error", { message: error })}</p>
  {:else if !brief}
    <p class="muted">{t("common.loading")}</p>
  {:else}
    <header>
      <h1>{dayName(brief.today, i18n.lang, true)}</h1>
      <p class="muted tnum">
        {brief.now_local.slice(11, 16)} · {t("today.dayEnds", { time: brief.day_rollover })}
      </p>
      {#if total > 0}
        <p class="summary">
          {t("today.summary", { overdue: brief.overdue.length, today: brief.due_today.length, week: brief.next_7_days.length })}
        </p>
      {/if}
    </header>

    {#if total === 0}
      <div class="empty">{t("today.empty", { shortcut: prettyShortcut(ui.shortcut || "CommandOrControl+Shift+Space") })}</div>
    {/if}

    {#if brief.overdue.length}
      <div class="group overdue">
        <h2 class="label">{t("today.overdue")}</h2>
        {#each brief.overdue as item (item.id)}<ItemRow {item} />{/each}
      </div>
    {/if}

    {#if brief.due_today.length}
      <div class="group">
        <h2 class="label">{t("today.today")}</h2>
        {#each brief.due_today as item (item.id)}<ItemRow {item} />{/each}
      </div>
    {:else if total > 0}
      <p class="muted calm">{t("today.clear")}</p>
    {/if}

    {#each groups as g (g.day)}
      <div class="group">
        <h2 class="label">{heading(g.day)}</h2>
        {#each g.items as item (item.id)}<ItemRow {item} />{/each}
      </div>
    {/each}

    {#if plan && (sessions.length || plan.at_risk.length) && total > 0}
      <div class="plan">
        <h2 class="label">{t("today.plan")}</h2>
        <p class="muted small">{t("today.planHint")}</p>
        <ul>
          {#each sessions as s (s.item_id)}
            <li>
              <button class="ghost link" onclick={() => (ui.selected = s.item_id)}>
                <span class="tnum when">{s.start_local.replace(/^(\w+) \d+ \w+ /, "$1 ")}–{s.end_local.slice(-5)}</span>
                <span>{s.title}</span>
                <span class="muted small">{s.block_label}</span>
              </button>
            </li>
          {/each}
        </ul>
        {#if plan.at_risk.length}
          <h3 class="label risk">{t("today.atRisk")}</h3>
          <ul class="risky">
            {#each plan.at_risk as r (r.item_id)}
              <li>
                <button class="ghost link" onclick={() => (ui.selected = r.item_id)}>
                  <span>{r.title}</span>
                  <span class="muted small">{r.reason}</span>
                </button>
              </li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  {/if}
</section>

<style>
  .page {
    max-width: 760px;
    margin: 0 auto;
  }
  header {
    margin-bottom: 30px;
  }
  h1 {
    font-size: 30px;
    font-weight: 650;
  }
  .summary {
    margin: 6px 0 0;
    color: var(--ink-2);
  }
  .group {
    margin: 0 -14px 26px;
  }
  .group > h2 {
    padding: 0 14px 6px;
  }
  .group.overdue > h2 {
    color: var(--accent);
  }
  .calm {
    margin: 0 0 26px;
  }
  .empty {
    padding: 36px 28px;
    border: 1px dashed var(--line);
    border-radius: var(--radius);
    color: var(--muted);
    text-align: center;
  }
  .plan {
    margin-top: 40px;
    padding-top: 24px;
    border-top: 1px solid var(--line);
  }
  .plan p {
    margin: 2px 0 10px;
  }
  .plan ul {
    list-style: none;
    margin: 0 -12px;
    padding: 0;
  }
  .link {
    width: 100%;
    display: grid;
    grid-template-columns: 130px 1fr auto;
    gap: 12px;
    text-align: left;
    padding: 7px 12px;
    align-items: baseline;
  }
  .when {
    color: var(--ink-2);
  }
  .risk {
    margin-top: 18px;
    margin-bottom: 4px;
    color: var(--accent);
  }
  .risky .link {
    grid-template-columns: 1fr;
    gap: 0;
  }
</style>

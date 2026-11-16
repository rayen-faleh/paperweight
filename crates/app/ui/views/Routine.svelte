<script lang="ts">
  import { api, errorText, type RoutineBlock } from "../lib/api";
  import { t } from "../lib/i18n.svelte";
  import { toast } from "../lib/state.svelte";

  let blocks = $state<RoutineBlock[]>([]);
  let error = $state<string | null>(null);
  let dirty = $state(false);

  async function load() {
    try {
      blocks = await api.getRoutine();
      dirty = false;
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }
  load();

  const days = [0, 1, 2, 3, 4, 5, 6];
  const byDay = (d: number) => blocks.filter((b) => b.weekday === d).sort((a, b) => sortKey(a) - sortKey(b));
  // Blocks after midnight (e.g. 00:30) sort after the evening, since they belong to the same day.
  const sortKey = (b: RoutineBlock) => {
    const [h, m] = b.start.split(":").map(Number);
    return (h < 2 ? h + 24 : h) * 60 + m;
  };

  function add(weekday: number) {
    blocks.push({ id: 0, weekday, start: "10:00", end: "11:00", label: "", available_for_tasks: true });
    dirty = true;
  }

  function removeBlock(b: RoutineBlock) {
    blocks = blocks.filter((x) => x !== b);
    dirty = true;
  }

  function copyMonday() {
    const monday = byDay(0);
    blocks = days.flatMap((d) => monday.map((b) => ({ ...b, id: 0, weekday: d })));
    dirty = true;
  }

  async function save() {
    try {
      blocks = await api.setRoutine(blocks.map((b) => ({ ...b, label: b.label.trim() || "Free time" })));
      dirty = false;
      toast(t("routine.saved"));
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }
</script>

<section class="page">
  <div class="head">
    <div>
      <h1>{t("routine.title")}</h1>
      <p class="muted">{t("routine.hint")}</p>
    </div>
    <div class="buttons">
      <button class="ghost" onclick={copyMonday}>{t("routine.copyAll")}</button>
      <button class="primary" disabled={!dirty} onclick={save}>{t("routine.save")}</button>
    </div>
  </div>
  {#if error}<p class="muted">{t("common.error", { message: error })}</p>{/if}

  <div class="days">
    {#each days as d (d)}
      <div class="day">
        <h2 class="label">{t(`weekday.${d}` as "weekday.0")}</h2>
        {#each byDay(d) as b, i (i + b.start + b.label)}
          <div class="block" class:free={b.available_for_tasks}>
            <div class="times">
              <input type="time" bind:value={b.start} oninput={() => (dirty = true)} aria-label={t("routine.start")} />
              <span class="muted">–</span>
              <input type="time" bind:value={b.end} oninput={() => (dirty = true)} aria-label={t("routine.end")} />
            </div>
            <input class="lbl" bind:value={b.label} oninput={() => (dirty = true)} placeholder={t("routine.label")} />
            <div class="foot">
              <label class="small">
                <input class="check" type="checkbox" bind:checked={b.available_for_tasks} onchange={() => (dirty = true)} />
                {t("routine.forTasks")}
              </label>
              <button class="ghost x" aria-label={t("routine.remove")} onclick={() => removeBlock(b)}>✕</button>
            </div>
          </div>
        {/each}
        <button class="ghost addb small" onclick={() => add(d)}>+ {t("routine.add")}</button>
      </div>
    {/each}
  </div>
</section>

<style>
  .page {
    max-width: 1200px;
    margin: 0 auto;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 20px;
    margin-bottom: 24px;
  }
  .head p {
    margin: 6px 0 0;
    max-width: 560px;
  }
  h1 {
    font-size: 26px;
  }
  .buttons {
    display: flex;
    gap: 8px;
    flex: none;
  }
  .days {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(190px, 1fr));
    gap: 14px;
  }
  .day {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .block {
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: transparent;
  }
  .block.free {
    background: var(--surface);
    box-shadow: inset 3px 0 0 var(--accent);
  }
  .times {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .times input {
    padding: 4px 6px;
    font-size: 13px;
  }
  .lbl {
    padding: 4px 6px;
    font-size: 13px;
  }
  .foot {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
  .foot label {
    display: flex;
    gap: 6px;
    align-items: center;
    color: var(--ink-2);
  }
  .x {
    padding: 2px 6px;
    font-size: 11px;
  }
  .addb {
    align-self: flex-start;
    color: var(--muted);
  }
  @media (max-width: 720px) {
    .head {
      flex-direction: column;
    }
  }
</style>

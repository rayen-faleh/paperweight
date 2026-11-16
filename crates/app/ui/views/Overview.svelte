<script lang="ts">
  // Overview: a period of your choosing (1 week to 1 year), shown as Stacks
  // or as Near and far. Replaces the old flat "All items" list.
  import { tick } from "svelte";
  import { api, errorText, KINDS, type Item, type Kind, type RoutineBlock } from "../lib/api";
  import { i18n, t } from "../lib/i18n.svelte";
  import { ui } from "../lib/state.svelte";
  import { capture, morph } from "../lib/flip";
  import { STOPS, toDays, toVal, offset, freeByWeekday, hours, fmtDay } from "../lib/timeline";
  import Stacks from "./overview/Stacks.svelte";
  import NearFar from "./overview/NearFar.svelte";

  type Mode = "stacks" | "nearfar";

  function remembered<T>(key: string, fallback: T, ok: (v: unknown) => boolean): T {
    try {
      const raw = localStorage.getItem(key);
      if (raw !== null) {
        const v = JSON.parse(raw);
        if (ok(v)) return v as T;
      }
    } catch {
      /* private window or blocked storage: use the default */
    }
    return fallback;
  }
  function remember(key: string, v: unknown) {
    try {
      localStorage.setItem(key, JSON.stringify(v));
    } catch {
      /* not essential */
    }
  }

  let items = $state<Item[]>([]);
  let today = $state<string | null>(null);
  let routine = $state<RoutineBlock[]>([]);
  let error = $state<string | null>(null);

  let mode = $state<Mode>(remembered("pw.overview.mode", "stacks", (v) => v === "stacks" || v === "nearfar"));
  // `value` drives the views; `thumb` is the slider's position, which glides
  // to a stop on its own when you click a stop or release near one.
  const startValue = remembered("pw.overview.range", toVal(14), (v) => typeof v === "number" && v >= 0 && v <= 1000);
  let value = $state<number>(startValue);
  let thumb = $state<number>(startValue);
  let dragging = $state(false);
  let query = $state("");
  let kind = $state<Kind | "">("");
  let stage: HTMLElement | undefined = $state();

  const range = $derived(toDays(value));

  $effect(() => {
    void ui.version;
    Promise.all([api.listItems("open", "", ""), api.brief(), api.getRoutine()]).then(
      ([list, brief, r]) => {
        items = list;
        today = brief.today;
        routine = r;
        error = null;
      },
      (e) => (error = errorText(e)),
    );
  });

  const entries = $derived(today ? items.map((item) => ({ item, d: offset(item.due_day, today!) })) : []);
  const free = $derived(freeByWeekday(routine));

  const q = $derived(query.trim().toLowerCase());
  function dim(it: Item): boolean {
    if (kind && it.kind !== kind) return true;
    if (!q) return false;
    const hay = [it.title, it.notes, it.source ?? "", ...it.steps.map((s) => s.title), ...it.requirements.map((r) => r.text)]
      .join("\n")
      .toLowerCase();
    return !hay.includes(q);
  }

  const inPeriod = $derived(entries.filter((e) => !e.item.overdue && e.d < range));
  const lateCount = $derived(entries.filter((e) => e.item.overdue).length);
  const matches = $derived(q || kind ? entries.filter((e) => !dim(e.item) && (e.item.overdue || e.d < range)).length : null);

  function periodName(days: number): string {
    if (days <= 8) return t("overview.next1w");
    if (days < 28) return t("overview.nextWeeks", { n: Math.round(days / 7) });
    if (days < 46) return t("overview.nextMonth");
    if (days < 330) return t("overview.nextMonths", { n: Math.round(days / 30.4) });
    return t("overview.nextYear");
  }

  // ---- slider ----
  let glide = 0;
  function glideThumb(target: number) {
    cancelAnimationFrame(glide);
    const from = thumb;
    const t0 = performance.now();
    const dur = 420;
    const step = (now: number) => {
      const k = Math.min(1, (now - t0) / dur);
      thumb = Math.round(from + (target - from) * (1 - Math.pow(1 - k, 3)));
      if (k < 1) glide = requestAnimationFrame(step);
    };
    glide = requestAnimationFrame(step);
  }
  function commit(v: number) {
    value = v;
    remember("pw.overview.range", v);
  }
  function onInput(e: Event) {
    cancelAnimationFrame(glide);
    thumb = +(e.currentTarget as HTMLInputElement).value;
    commit(thumb);
  }
  function release() {
    dragging = false;
    const near = STOPS.map(([d]) => toVal(d)).find((s) => Math.abs(s - thumb) < 24);
    if (near !== undefined && near !== thumb) jump(near);
  }
  function jump(v: number) {
    dragging = false;
    commit(v); // the views settle in one staggered move
    glideThumb(v); // while the thumb glides to meet them
  }
  const activeStop = $derived(STOPS.find(([d]) => Math.abs(d - range) / range < 0.06)?.[0]);

  // ---- design toggle: items morph from one layout to the other ----
  async function setMode(m: Mode) {
    if (m === mode) return;
    const snap = stage ? capture(stage) : null;
    mode = m;
    remember("pw.overview.mode", m);
    await tick();
    if (stage && snap) morph(stage, snap, { duration: 680, stagger: 10, maxStagger: 260 });
  }

  const open = (id: string) => (ui.selected = id);
</script>

<section class="page">
  <header class="top">
    <div class="titles">
      <h1>{t("nav.overview")}</h1>
      {#if today}
        <p class="muted tnum">
          {t("overview.summary", {
            from: fmtDay(today, 0, i18n.lang, { day: "numeric", month: "short" }),
            to: fmtDay(today, range - 1, i18n.lang, { day: "numeric", month: "short", year: "numeric" }),
            count: inPeriod.length,
            work: hours(inPeriod.reduce((s, e) => s + e.item.estimated_minutes, 0)),
          })}{#if lateCount}{" · "}<span class="lateinfo">{t("overview.lateCount", { n: lateCount })}</span>{/if}
        </p>
      {/if}
    </div>
    <div class="seg" role="radiogroup" aria-label={t("overview.design")}>
      <span class="pill" class:right={mode === "nearfar"} aria-hidden="true"></span>
      <button role="radio" aria-checked={mode === "stacks"} class:on={mode === "stacks"} onclick={() => setMode("stacks")}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="1.5" y="3" width="3.6" height="10" rx="1" /><rect x="6.2" y="6" width="3.6" height="7" rx="1" /><rect x="10.9" y="4.5" width="3.6" height="8.5" rx="1" /></svg>
        {t("overview.stacks")}
      </button>
      <button role="radio" aria-checked={mode === "nearfar"} class:on={mode === "nearfar"} onclick={() => setMode("nearfar")}>
        <svg viewBox="0 0 16 16" aria-hidden="true"><rect x="1.5" y="2" width="13" height="2.6" rx="1" /><rect x="1.5" y="6.6" width="9.5" height="2.1" rx="1" /><rect x="1.5" y="10.6" width="6.5" height="1.6" rx=".8" /><rect x="1.5" y="13.4" width="4" height="1.1" rx=".55" /></svg>
        {t("overview.nearfar")}
      </button>
    </div>
  </header>

  <div class="period">
    <div class="period-head">
      <label for="overview-range" class="pname">{periodName(toDays(thumb))}</label>
      <span class="muted small">{mode === "stacks" ? t("overview.stacksHint") : t("overview.nearfarHint")}</span>
    </div>
    <input
      id="overview-range"
      type="range"
      min="0"
      max="1000"
      step="1"
      value={thumb}
      aria-valuetext={`${periodName(range)}, ${range} days`}
      oninput={onInput}
      onpointerdown={() => (dragging = true)}
      onpointerup={release}
      onpointercancel={release}
    />
    <div class="stops">
      {#each STOPS as [d, key] (d)}
        <button type="button" class:on={activeStop === d} style:left={`${toVal(d) / 10}%`} onclick={() => jump(toVal(d))}>{t(key)}</button>
      {/each}
    </div>
  </div>

  <div class="filters">
    <input id="overview-search" type="text" placeholder={t("overview.search")} bind:value={query} autocomplete="off" />
    <div class="kinds">
      {#each KINDS as k (k)}
        <button type="button" class="chip" class:on={kind === k} aria-pressed={kind === k} onclick={() => (kind = kind === k ? "" : k)}>
          <i class="shape k-{k}"></i>{t(`kind.${k}`)}
        </button>
      {/each}
    </div>
    {#if matches !== null}<span class="small muted tnum">{t("overview.matches", { n: matches })}</span>{/if}
  </div>

  {#if error}
    <p class="muted">{t("common.error", { message: error })}</p>
  {:else if today}
    <div class="stage" bind:this={stage}>
      {#if mode === "stacks"}
        <Stacks {entries} {today} {range} {free} {dragging} {dim} onopen={open} />
      {:else}
        <NearFar {entries} {range} {dragging} {dim} onopen={open} />
      {/if}
    </div>
    {#if inPeriod.length === 0 && lateCount === 0}
      <p class="muted empty">{t("overview.empty")}</p>
    {/if}
  {:else}
    <p class="muted">{t("common.loading")}</p>
  {/if}
</section>

<style>
  .page {
    max-width: 1160px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .top {
    display: flex;
    justify-content: space-between;
    align-items: flex-end;
    gap: 16px;
    flex-wrap: wrap;
  }
  .titles {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }
  h1 {
    font-size: 26px;
  }
  .titles p {
    margin: 0;
  }
  .lateinfo {
    color: var(--accent);
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
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 7px;
    border: 0;
    background: transparent;
    padding: 7px 14px;
    font-size: 13.5px;
    color: var(--muted);
    white-space: nowrap;
    transition: color 0.2s;
  }
  .seg button.on {
    color: var(--ink);
    font-weight: 600;
  }
  .seg svg {
    width: 15px;
    height: 15px;
    fill: currentColor;
  }
  .period {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .period-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
    flex-wrap: wrap;
  }
  .pname {
    font-size: 17px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  input[type="range"] {
    width: 100%;
    appearance: none;
    -webkit-appearance: none;
    background: transparent;
    height: 30px;
    margin: 4px 0 0;
    padding: 0;
    border: 0;
    box-shadow: none;
    cursor: pointer;
  }
  input[type="range"]::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 4px;
    background: var(--line);
  }
  input[type="range"]::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 20px;
    height: 20px;
    margin-top: -8px;
    border-radius: 50%;
    background: var(--accent);
    border: 3px solid var(--surface);
    box-shadow: 0 1px 5px rgba(0, 0, 0, 0.25);
    transition: transform 0.15s;
  }
  input[type="range"]:active::-webkit-slider-thumb {
    transform: scale(1.15);
  }
  input[type="range"]::-moz-range-track {
    height: 4px;
    border-radius: 4px;
    background: var(--line);
  }
  input[type="range"]::-moz-range-thumb {
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent);
    border: 3px solid var(--surface);
  }
  input[type="range"]:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 3px;
    border-radius: 6px;
    box-shadow: none;
  }
  .stops {
    position: relative;
    height: 22px;
    margin: 0 10px;
  }
  .stops button {
    position: absolute;
    transform: translateX(-50%);
    border: 0;
    background: transparent;
    padding: 1px 6px;
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    transition: color 0.2s;
  }
  .stops button:hover,
  .stops button.on {
    color: var(--ink);
  }
  .stops button.on {
    font-weight: 600;
  }
  .filters {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .filters input {
    width: 240px;
    max-width: 100%;
  }
  .kinds {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .chip {
    display: inline-flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    padding: 4px 11px;
    border-radius: 99px;
    color: var(--muted);
  }
  .chip .shape {
    color: var(--muted);
  }
  .chip.on {
    border-color: var(--accent);
    color: var(--ink);
    background: var(--accent-soft);
  }
  .chip.on .shape {
    color: var(--ink);
  }
  .stage {
    position: relative;
    min-width: 0;
  }
  .empty {
    text-align: center;
    margin: 0;
  }
  @media (max-width: 640px) {
    .stops button {
      font-size: 11px;
      padding: 1px 2px;
    }
    .filters input {
      width: 100%;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .pill {
      transition: none;
    }
  }
</style>

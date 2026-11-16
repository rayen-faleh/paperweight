<script lang="ts">
  // Concept B: the period cut into columns (days, weeks or months); each
  // deadline is a slip on its column's pile, taller when it needs more work.
  import { untrack } from "svelte";
  import { capture, morph, type Snapshot } from "../../lib/flip";
  import { buckets, unitFor, freeMinutes, hours, fmtDay, type Bucket, type Unit } from "../../lib/timeline";
  import { localTime, type Item } from "../../lib/api";
  import { i18n, t } from "../../lib/i18n.svelte";

  type Entry = { item: Item; d: number };
  let {
    entries,
    today,
    range,
    free,
    dragging,
    dim,
    onopen,
  }: {
    entries: Entry[];
    today: string;
    range: number;
    free: number[];
    dragging: boolean;
    dim: (it: Item) => boolean;
    onopen: (id: string) => void;
  } = $props();

  let root: HTMLElement | undefined = $state();
  let width = $state(900);

  // Columns need about this much width to keep titles readable.
  const MIN_COL = 112;
  const LATE_W = 124;
  const usable = $derived(Math.max(240, width - LATE_W));
  // Days only while each day still gets a readable column; otherwise weeks.
  const unit = $derived.by((): Unit => {
    const u = unitFor(range);
    // Day columns hold short titles on two lines, so they may be narrower.
    return u === "day" && usable / range < 78 ? "week" : u;
  });

  function label(b: Bucket): { title: string; sub: string } {
    const lang = i18n.lang;
    if (b.unit === "day") {
      return {
        title: b.start === 0 ? t("overview.today") : fmtDay(today, b.start, lang, { weekday: "short", day: "numeric" }),
        sub: fmtDay(today, b.start, lang, { month: "short" }),
      };
    }
    if (b.unit === "week") {
      const span = `${fmtDay(today, b.start, lang, { day: "numeric", month: "short" })} – ${fmtDay(today, b.end - 1, lang, { day: "numeric", month: "short" })}`;
      if (b.start === 0) return { title: t("overview.thisWeek"), sub: span };
      if (b.key === `w${b.start}` && b.start <= 7 && b.start > 0) return { title: t("overview.nextWeek"), sub: span };
      return { title: t("overview.weekOf", { date: fmtDay(today, b.start, lang, { day: "numeric", month: "short" }) }), sub: span };
    }
    return {
      title: fmtDay(today, b.start, lang, { month: "long" }),
      sub: fmtDay(today, b.start, lang, { year: "numeric" }),
    };
  }

  interface Col {
    key: string;
    title: string;
    sub: string;
    late?: boolean;
    current?: boolean;
    entries: Entry[];
    work: number;
    free: number;
  }

  const cols = $derived.by((): Col[] => {
    const late = entries.filter((e) => e.item.overdue);
    const rest = entries.filter((e) => !e.item.overdue);
    const out: Col[] = [];
    if (late.length) {
      out.push({
        key: "late",
        title: t("overview.overdue"),
        sub: late.length === 1 ? t("overview.count1") : t("overview.count", { n: late.length }),
        late: true,
        entries: late,
        work: late.reduce((s, e) => s + e.item.estimated_minutes, 0),
        free: 0,
      });
    }
    for (const b of buckets(today, range, unit)) {
      const es = rest.filter((e) => e.d >= b.start && e.d < b.end);
      out.push({
        key: b.key,
        ...label(b),
        current: b.start === 0,
        entries: es,
        work: es.reduce((s, e) => s + e.item.estimated_minutes, 0),
        free: freeMinutes(free, today, b.start, b.end),
      });
    }
    return out;
  });

  const lateCol = $derived(cols.find((c) => c.late));
  const dated = $derived(cols.filter((c) => !c.late));
  // Too many buckets for one row: wrap them onto balanced shelves.
  const perRow = $derived.by(() => {
    const n = Math.max(1, dated.length);
    const fit = Math.max(1, Math.floor(usable / MIN_COL));
    if (n <= fit) return n;
    const rows = Math.ceil(n / fit);
    return Math.ceil(n / rows);
  });
  const shelves = $derived(Math.ceil(dated.length / perRow));
  const compact = false;
  const limit = $derived(shelves > 1 ? 4 : 6);

  // Animate whenever what is on screen changes: capture before the DOM
  // update, morph after it. Dragging gets short, direct moves; a release or
  // a jump gets the full staggered settle.
  const signature = $derived(
    cols.map((c) => c.key + ":" + c.entries.slice(0, limit).map((e) => e.item.id).join(".")).join("|") + `|${compact}|${Math.round(width / 40)}`,
  );
  // Only the signature is tracked: the element binding arriving or the drag
  // state changing must not restart (or cancel) animations on their own.
  let snap: Snapshot | null = null;
  $effect.pre(() => {
    void signature;
    const el = untrack(() => root);
    if (el) snap = capture(el);
  });
  $effect(() => {
    void signature;
    const el = untrack(() => root);
    const fast = untrack(() => dragging);
    if (el && snap) {
      morph(el, snap, fast ? { duration: 300, stagger: 0 } : { duration: 620, stagger: 12, maxStagger: 240 });
      snap = null;
    }
  });

  const slipHeight = (m: number) => 26 + Math.round((Math.min(m, 480) / 480) * 30);
  const when = (it: Item) => (it.all_day ? "" : localTime(it.due_local));
</script>

{#snippet column(c: Col)}
    <section class="col" class:late={c.late} class:current={c.current} class:compact>
      <header data-flip={`h:${c.key}`}>
        <b>{c.title}</b>
        <span>{c.sub}</span>
      </header>
      <div class="pile">
        {#each c.entries.slice(0, limit) as e (e.item.id)}
          <button
            class="slip"
            class:late={e.item.overdue}
            class:dim={dim(e.item)}
            data-flip={e.item.id}
            style:min-height={`${slipHeight(e.item.estimated_minutes)}px`}
            title={`${e.item.title} — ${e.item.due_label}`}
            onclick={() => onopen(e.item.id)}
          >
            <i class="shape k-{e.item.kind}" class:late={e.item.overdue}></i>
            {#if !compact}
              <span class="t">{e.item.title}</span>
              {#if when(e.item)}<span class="w">{when(e.item)}</span>{/if}
            {/if}
          </button>
        {/each}
        {#if c.entries.length > limit}
          <span class="more" data-flip={`m:${c.key}`}>{t("overview.more", { n: c.entries.length - limit })}</span>
        {/if}
      </div>
      <footer data-flip={`f:${c.key}`} title={c.late ? "" : t("overview.load", { work: hours(c.work), free: hours(c.free) })}>
        <span class="tnum">{c.work ? hours(c.work) : "—"}</span>
        {#if !c.late}
          <div class="bar"><i class:hot={c.work > c.free} style:width={`${Math.min(100, (c.work / Math.max(c.free, 1)) * 100)}%`}></i></div>
        {/if}
      </footer>
    </section>
{/snippet}

<div class="stacks" bind:this={root} bind:clientWidth={width}>
  {#if lateCol}{@render column(lateCol)}{/if}
  <div class="shelves" style:grid-template-columns={`repeat(${perRow}, minmax(0, 1fr))`}>
    {#each dated as c (c.key)}{@render column(c)}{/each}
  </div>
</div>

<style>
  .stacks {
    display: flex;
    min-height: 380px;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 14px;
    overflow: hidden;
    animation: chrome-in 0.45s ease-out;
  }
  @keyframes chrome-in {
    from {
      background-color: transparent;
      border-color: transparent;
    }
  }
  @keyframes col-in {
    from {
      background-color: transparent;
      box-shadow: none;
    }
  }
  .shelves {
    flex: 1;
    min-width: 0;
    display: grid;
    grid-auto-rows: minmax(150px, auto);
  }
  .col {
    min-width: 0;
    display: flex;
    flex-direction: column;
    box-shadow:
      inset -1px 0 0 var(--line),
      inset 0 -1px 0 var(--line);
    animation: col-in 0.4s ease-out;
  }
  .col.late {
    flex: 0 0 124px;
    background: var(--accent-soft);
    box-shadow: inset -1px 0 0 var(--line);
  }
  .col.current {
    background: color-mix(in srgb, var(--sunken) 55%, transparent);
  }
  header {
    padding: 12px 9px 8px;
    display: flex;
    flex-direction: column;
    min-height: 52px;
    min-width: 0;
  }
  header b {
    font-size: 13px;
    font-weight: 650;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  header span {
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .current header b,
  .late header b {
    color: var(--accent);
  }
  .compact header {
    padding-inline: 4px;
    align-items: center;
    text-align: center;
  }
  .pile {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 4px 7px 8px;
  }
  .compact .pile {
    padding-inline: 4px;
    align-items: stretch;
  }
  .slip {
    display: grid;
    grid-template-columns: auto 1fr;
    grid-template-rows: auto auto;
    column-gap: 7px;
    align-content: start;
    text-align: left;
    padding: 6px 8px;
    border-radius: 7px;
    background: var(--bg);
    border: 1px solid var(--line);
    font-size: 12.5px;
    line-height: 1.3;
    cursor: pointer;
    box-shadow: 0 1px 0 rgba(31, 29, 26, 0.04);
    transition:
      opacity 0.35s,
      border-color 0.2s,
      box-shadow 0.2s,
      translate 0.2s;
    will-change: transform;
  }
  .slip:hover,
  .slip:focus-visible {
    border-color: var(--muted);
    box-shadow: 0 6px 16px rgba(31, 29, 26, 0.1);
    translate: 0 -1px;
  }
  .slip .shape {
    grid-row: 1 / span 2;
    margin-top: 3px;
  }
  .slip .t {
    min-width: 0;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow-wrap: anywhere;
  }
  .slip .w {
    grid-column: 2;
    font-size: 11px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }
  .slip.late {
    border-color: color-mix(in srgb, var(--accent) 45%, var(--line));
  }
  .slip.late .t {
    color: var(--accent);
  }
  .compact .slip {
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 7px 0;
  }
  .compact .slip .shape {
    margin-top: 0;
  }
  .slip.dim {
    opacity: 0.2;
  }
  .more {
    font-size: 11.5px;
    color: var(--muted);
    padding: 2px 4px;
    text-align: center;
  }
  footer {
    padding: 6px 9px 12px;
    font-size: 11.5px;
    color: var(--muted);
    display: flex;
    flex-direction: column;
    gap: 5px;
  }
  .compact footer {
    padding-inline: 5px;
    align-items: center;
  }
  .bar {
    height: 4px;
    width: 100%;
    border-radius: 3px;
    background: var(--sunken);
    overflow: hidden;
  }
  .bar i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: var(--ink-2);
    opacity: 0.5;
    transition: width 0.55s cubic-bezier(0.22, 0.8, 0.24, 1);
  }
  .bar i.hot {
    background: var(--accent);
    opacity: 0.9;
  }
  @media (prefers-reduced-motion: reduce) {
    .bar i,
    .slip {
      transition: none;
    }
    .stacks,
    .col {
      animation: none;
    }
  }
</style>

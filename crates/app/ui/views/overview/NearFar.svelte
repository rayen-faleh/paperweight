<script lang="ts">
  // Concept C: time runs down the page on a logarithmic scale. Tomorrow gets
  // plenty of room, next year a sliver; near items are large and dark, far
  // ones smaller and fainter. The slider moves only the horizon.
  import type { Item } from "../../lib/api";
  import { t } from "../../lib/i18n.svelte";
  import { EASE_OUT, EASE_SETTLE } from "../../lib/flip";
  import type { Key } from "../../lib/strings";

  type Entry = { item: Item; d: number };
  let {
    entries,
    range,
    dragging,
    dim,
    onopen,
  }: {
    entries: Entry[];
    range: number;
    dragging: boolean;
    dim: (it: Item) => boolean;
    onopen: (id: string) => void;
  } = $props();

  const K = 1.4; // days; smaller means more room for the first few days
  const SPAN = 470; // height of the period, in px
  const ROW = 32;
  const LATE_ROW = 30;

  const late = $derived(entries.filter((e) => e.item.overdue));
  const top = $derived(late.length ? 24 + late.length * LATE_ROW + 26 : 18);
  const frac = (d: number) => Math.log(1 + Math.max(d, 0) / K) / Math.log(1 + range / K);
  const yOf = (d: number) => top + SPAN * frac(d);

  interface Row {
    item: Item;
    y: number;
    scale: number;
    opacity: number;
    shown: boolean;
    order: number;
  }

  // Empty stretches of time are compressed: a gap keeps its first 44px and
  // a fifth of the rest, so a big jump still looks bigger without leaving
  // acres of blank paper. Guides go through the same mapping, so they stay
  // between the right rows.
  const squeeze = (gap: number) => (gap <= 44 ? gap : 44 + (gap - 44) * 0.2);

  const layout = $derived.by(() => {
    const rows: Row[] = [];
    const pts: [number, number][] = [[top, top]]; // ideal y -> drawn y
    let next = top - 6;
    let order = 0;
    late.forEach((e, i) => {
      rows.push({ item: e.item, y: 24 + i * LATE_ROW, scale: 1, opacity: 1, shown: true, order: order++ });
    });
    for (const e of entries) {
      if (e.item.overdue) continue;
      if (e.d >= range) {
        rows.push({ item: e.item, y: -1, scale: 0.8, opacity: 0, shown: false, order: 0 });
        continue;
      }
      const f = frac(e.d);
      const scale = 1.06 - 0.34 * f;
      const h = ROW * scale;
      const ideal = yOf(e.d);
      const [pi, po] = pts[pts.length - 1];
      const centre = Math.max(po + squeeze(ideal - pi), next + h / 2);
      pts.push([ideal, centre]);
      next = centre + h / 2;
      rows.push({ item: e.item, y: centre - h / 2, scale, opacity: 1 - 0.5 * f, shown: true, order: order++ });
    }
    const [pi, po] = pts[pts.length - 1];
    const end = Math.max(po + squeeze(top + SPAN - pi), next + 8);
    pts.push([top + SPAN + 1e-6, end]);
    for (const r of rows) if (!r.shown) r.y = end + 40;
    const map = (y: number) => {
      for (let i = 1; i < pts.length; i++) {
        const [a, a2] = pts[i - 1];
        const [b, b2] = pts[i];
        if (y <= b) return b === a ? a2 : a2 + ((y - a) / (b - a)) * (b2 - a2);
      }
      return end;
    };
    return { rows, map, height: end + 36 };
  });

  const GUIDES: [number, Key][] = [
    [0, "overview.today"],
    [1, "overview.tomorrow"],
    [7, "overview.inWeek"],
    [14, "overview.in2Weeks"],
    [30, "overview.inMonth"],
    [61, "overview.in2Months"],
    [91, "overview.in3Months"],
    [182, "overview.in6Months"],
    [365, "overview.inYear"],
  ];
  const guides = $derived.by(() => {
    let lastY = -1e9;
    return GUIDES.map(([d, key]) => {
      const inside = d <= range;
      const y = layout.map(yOf(Math.min(d, range)));
      const shown = inside && y - lastY >= 22;
      if (shown) lastY = y;
      return { d, key, y, shown };
    });
  });

  // Dragging: short and direct, so items track the hand. Settling: longer,
  // staggered from the top down, with the soft-landing ease.
  const motion = $derived(
    dragging ? { dur: 240, stagger: 0, ease: EASE_OUT } : { dur: 600, stagger: 6, ease: EASE_SETTLE },
  );
</script>

<div class="nf" style:height={`${layout.height}px`} style:--dur={`${motion.dur}ms`} style:--ease={motion.ease}>
  <div class="rail" aria-hidden="true">
    {#if late.length}<span class="guide late-g" style:transform="translateY(6px)">{t("overview.behind")}</span>{/if}
    {#each guides as g (g.d)}
      <span class="guide" class:today={g.d === 0} style:transform={`translateY(${g.y - 9}px)`} style:opacity={g.shown ? 1 : 0}>
        {t(g.key)}
      </span>
      <i class="rule" class:today={g.d === 0} style:transform={`translateY(${g.y}px)`} style:opacity={g.shown ? 1 : 0}></i>
    {/each}
  </div>
  {#each layout.rows as r (r.item.id)}
    <button
      class="row"
      class:late={r.item.overdue}
      class:dim={dim(r.item)}
      data-flip={r.item.id}
      tabindex={r.shown ? 0 : -1}
      aria-hidden={!r.shown}
      style:transform={`translateY(${r.y.toFixed(1)}px) scale(${r.scale.toFixed(3)})`}
      style:opacity={r.shown ? r.opacity.toFixed(2) : "0"}
      style:pointer-events={r.shown ? "auto" : "none"}
      style:transition-delay={`${Math.min(r.order * motion.stagger, 120)}ms`}
      onclick={() => onopen(r.item.id)}
    >
      <i class="shape k-{r.item.kind}" class:late={r.item.overdue}></i>
      <span class="t">{r.item.title}</span>
      <span class="d">{r.item.due_label.replace(/^overdue \((.*)\)$/, "$1")}</span>
      {#if r.item.requirements.length}
        <span class="m">{t("row.needs", { done: r.item.requirements_done, total: r.item.requirements.length })}</span>
      {/if}
    </button>
  {/each}
</div>

<style>
  .nf {
    position: relative;
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 14px;
    overflow: hidden;
    transition: height var(--dur) var(--ease);
    animation: chrome-in 0.45s ease-out;
  }
  .rail {
    position: absolute;
    inset: 0;
    pointer-events: none;
    animation: fade-in 0.5s ease-out;
  }
  @keyframes chrome-in {
    from {
      background-color: transparent;
      border-color: transparent;
    }
  }
  @keyframes fade-in {
    from {
      opacity: 0;
    }
  }
  .rail::before {
    content: "";
    position: absolute;
    left: 132px;
    top: 0;
    bottom: 0;
    width: 1px;
    background: var(--line);
  }
  .guide {
    position: absolute;
    left: 16px;
    top: 0;
    width: 104px;
    font-size: 12px;
    color: var(--muted);
    white-space: nowrap;
    transition:
      transform var(--dur) var(--ease),
      opacity 0.3s;
  }
  .guide.today {
    color: var(--accent);
    font-weight: 650;
  }
  .late-g {
    color: var(--accent);
    font-weight: 650;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    font-size: 11px;
  }
  .rule {
    position: absolute;
    left: 132px;
    right: 0;
    top: 0;
    height: 1px;
    background: var(--line);
    opacity: 0.6;
    transition:
      transform var(--dur) var(--ease),
      opacity 0.3s;
  }
  .rule.today {
    background: var(--accent);
    opacity: 0.45;
  }
  .row {
    position: absolute;
    left: 150px;
    top: 0;
    max-width: calc(100% - 166px);
    display: flex;
    align-items: baseline;
    gap: 9px;
    border: 0;
    background: transparent;
    padding: 4px 8px;
    margin-left: -8px;
    border-radius: 7px;
    text-align: left;
    white-space: nowrap;
    cursor: pointer;
    transform-origin: 0 50%;
    transition:
      transform var(--dur) var(--ease),
      opacity calc(var(--dur) * 0.8) ease,
      background 0.15s;
    will-change: transform;
  }
  .row:hover,
  .row:focus-visible {
    background: var(--sunken);
  }
  .row .shape {
    align-self: center;
  }
  .t {
    font-size: 15px;
    font-weight: 560;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .d {
    font-size: 12.5px;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
    flex: none;
  }
  .m {
    font-size: 12px;
    color: var(--muted);
    flex: none;
  }
  .row.late .t {
    color: var(--accent);
  }
  .row.dim {
    opacity: 0.16 !important;
  }
  @media (max-width: 640px) {
    .rail::before,
    .rule {
      left: 92px;
    }
    .guide {
      width: 70px;
      font-size: 11px;
      overflow: hidden;
      text-overflow: ellipsis;
    }
    .row {
      left: 106px;
      max-width: calc(100% - 116px);
    }
    .m {
      display: none;
    }
  }
  @media (prefers-reduced-motion: reduce) {
    .nf,
    .guide,
    .rule,
    .row,
    .rail {
      transition: none;
      animation: none;
    }
  }
</style>

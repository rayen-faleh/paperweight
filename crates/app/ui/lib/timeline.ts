// Time math for the Overview: the period slider, buckets (day / week / month)
// and free task time from the routine. All days are logical days
// ("YYYY-MM-DD" from the backend, already rollover-aware), handled in UTC so
// the browser's own timezone never shifts them.
import type { RoutineBlock } from "./api";

export const DAY = 86_400_000;
export const MIN_DAYS = 7;
export const MAX_DAYS = 365;

/** The slider is logarithmic: as much travel for 1–2 weeks as for 6–12 months. */
export const toDays = (v: number) => Math.round(MIN_DAYS * Math.pow(MAX_DAYS / MIN_DAYS, v / 1000));
export const toVal = (d: number) => Math.round((1000 * Math.log(d / MIN_DAYS)) / Math.log(MAX_DAYS / MIN_DAYS));

export const STOPS = [
  [7, "overview.stop.1w"],
  [14, "overview.stop.2w"],
  [30, "overview.stop.1m"],
  [61, "overview.stop.2m"],
  [182, "overview.stop.6m"],
  [365, "overview.stop.1y"],
] as const;

export type Unit = "day" | "week" | "month";
export const unitFor = (range: number): Unit => (range <= 21 ? "day" : range <= 100 ? "week" : "month");

const ms = (day: string) => Date.parse(`${day}T00:00:00Z`);
/** Whole days from `today` to `day`. */
export const offset = (day: string, today: string) => Math.round((ms(day) - ms(today)) / DAY);
export const dateAt = (today: string, n: number) => new Date(ms(today) + n * DAY);

export interface Bucket {
  key: string;
  start: number; // inclusive day offset, clipped to the period
  end: number; // exclusive
  unit: Unit;
}

export function buckets(today: string, range: number, unit: Unit): Bucket[] {
  const out: Bucket[] = [];
  if (unit === "day") {
    for (let d = 0; d < range; d++) out.push({ key: `d${d}`, start: d, end: d + 1, unit });
    return out;
  }
  if (unit === "week") {
    const dow = (dateAt(today, 0).getUTCDay() + 6) % 7; // Monday = 0
    for (let s = -dow; s < range; s += 7) out.push({ key: `w${s}`, start: Math.max(s, 0), end: Math.min(s + 7, range), unit });
    return out;
  }
  const t = dateAt(today, 0);
  let y = t.getUTCFullYear();
  let m = t.getUTCMonth();
  for (;;) {
    const s = Math.round((Date.UTC(y, m, 1) - ms(today)) / DAY);
    const e = Math.round((Date.UTC(y, m + 1, 1) - ms(today)) / DAY);
    if (s >= range) break;
    out.push({ key: `m${y}-${m}`, start: Math.max(s, 0), end: Math.min(e, range), unit });
    m += 1;
    if (m > 11) {
      m = 0;
      y += 1;
    }
  }
  return out;
}

function minutes(hhmm: string): number {
  const [h, m] = hhmm.split(":").map(Number);
  return h * 60 + m;
}

/** Free task minutes per weekday (Monday = 0) from blocks marked "for tasks". */
export function freeByWeekday(routine: RoutineBlock[]): number[] {
  const out = [0, 0, 0, 0, 0, 0, 0];
  for (const b of routine) {
    if (!b.available_for_tasks) continue;
    let len = minutes(b.end) - minutes(b.start);
    if (len <= 0) len += 24 * 60;
    out[b.weekday] += len;
  }
  return out;
}

export function freeMinutes(perWeekday: number[], today: string, start: number, end: number): number {
  let total = 0;
  for (let d = start; d < end; d++) total += perWeekday[(dateAt(today, d).getUTCDay() + 6) % 7];
  return total;
}

export function hours(m: number): string {
  if (m <= 0) return "0h";
  if (m < 60) return `${m}m`;
  const h = Math.floor(m / 60);
  const r = Math.round(m % 60);
  return r ? `${h}h ${r}m` : `${h}h`;
}

export function locale(lang: string): string {
  return lang === "ja" ? "ja-JP" : "en-GB";
}

export function fmtDay(today: string, d: number, lang: string, opts: Intl.DateTimeFormatOptions): string {
  return dateAt(today, d).toLocaleDateString(locale(lang), { timeZone: "UTC", ...opts });
}

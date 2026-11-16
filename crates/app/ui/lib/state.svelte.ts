export type View = "today" | "overview" | "routine" | "archive" | "settings";

export const ui = $state({
  view: "today" as View,
  /** The item open in the detail panel. */
  selected: null as string | null,
  quickAdd: false,
  /** Bumped whenever the database changes (from MCP, the scheduler, or here). */
  version: 0,
  toast: null as { text: string; at: number } | null,
  shortcut: "",
});

export function toast(text: string) {
  ui.toast = { text, at: Date.now() };
  const at = ui.toast.at;
  setTimeout(() => {
    if (ui.toast?.at === at) ui.toast = null;
  }, 4000);
}

export function changed() {
  ui.version += 1;
}

/** "Fri 20 Nov" from a YYYY-MM-DD logical day, independent of the browser timezone. */
export function dayName(day: string, lang: string, long = false): string {
  return new Date(`${day}T00:00:00Z`).toLocaleDateString(lang === "ja" ? "ja-JP" : "en-GB", {
    weekday: long ? "long" : "short",
    day: "numeric",
    month: "short",
    timeZone: "UTC",
  });
}

export function addDays(day: string, n: number): string {
  const d = new Date(`${day}T00:00:00Z`);
  d.setUTCDate(d.getUTCDate() + n);
  return d.toISOString().slice(0, 10);
}

/** Human form of a shortcut string such as CommandOrControl+Shift+Space. */
export function prettyShortcut(s: string): string {
  const mac = navigator.userAgent.includes("Mac");
  return s
    .replace(/CommandOrControl|CmdOrCtrl/g, mac ? "⌘" : "Ctrl")
    .replace(/Shift/g, mac ? "⇧" : "Shift")
    .replace(/Alt|Option/g, mac ? "⌥" : "Alt")
    .replace(/\+/g, mac ? "" : "+");
}

/** A calendar date in the interface language, e.g. "16 Nov 2026". */
export function shortDate(iso: string, lang: string): string {
  return new Date(iso).toLocaleDateString(lang === "ja" ? "ja-JP" : "en-GB", { day: "numeric", month: "short", year: "numeric" });
}

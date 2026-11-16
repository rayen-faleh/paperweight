import { invoke } from "@tauri-apps/api/core";

export type Kind = "paperwork" | "assignment" | "payment" | "appointment" | "social" | "other";
export const KINDS: Kind[] = ["paperwork", "assignment", "payment", "appointment", "social", "other"];
export type Status = "open" | "done" | "archived";
export type Freq = "none" | "daily" | "weekly" | "monthly" | "yearly";

export interface Step { id: number; title: string; done: boolean; soft_due_at: string | null; position: number }
export interface Requirement { id: number; text: string; done: boolean; position: number }
export interface Recurrence { freq: Freq; interval: number }

export interface Item {
  id: string;
  title: string;
  kind: Kind;
  due_at: string;
  all_day: boolean;
  status: Status;
  notes: string;
  source: string | null;
  estimated_minutes: number;
  snoozed_until: string | null;
  recurrence: Recurrence;
  steps: Step[];
  requirements: Requirement[];
  created_at: string;
  updated_at: string;
  completed_at: string | null;
  archived_at: string | null;
  due_local: string;
  due_day: string;
  due_label: string;
  overdue: boolean;
  snoozed: boolean;
  steps_done: number;
  requirements_done: number;
}

export interface Brief {
  now_local: string;
  timezone: string;
  today: string;
  day_rollover: string;
  overdue: Item[];
  due_today: Item[];
  next_7_days: Item[];
}

export interface Session {
  item_id: string; title: string; start: string; end: string;
  start_local: string; end_local: string; block_label: string; day: string; minutes: number;
}
export interface Unplaced { item_id: string; title: string; due_at: string; estimated_minutes: number; reason: string }
export interface WeekPlan { sessions: Session[]; at_risk: Unplaced[]; later: Unplaced[] }

export interface RoutineBlock { id: number; weekday: number; start: string; end: string; label: string; available_for_tasks: boolean }

export interface Settings {
  timezone: string; day_rollover: string; quiet_start: string; quiet_end: string;
  reminder_offsets: string[]; feed_port: number; language: "en" | "ja"; quick_add_shortcut: string;
}
export interface SettingsInfo {
  settings: Settings; effective_timezone: string; db_path: string; log_path: string;
  feed_url: string; feed_error: string | null; shortcut_error: string | null; autostart: boolean;
}

export interface ConnectInfo {
  binary: string | null; snippet: string | null; claude_config_path: string | null;
  claude_config_exists: boolean; db_path: string;
}
export type ConnectOutcome =
  | { outcome: "written"; path: string; backup: string; result: string; replaced: boolean }
  | { outcome: "manual"; reason: string; steps: string[] };

export interface Changes {
  title?: string; kind?: Kind; due?: string; notes?: string; source?: string;
  estimated_minutes?: number; recurrence?: Recurrence; status?: "open" | "done"; clear_snooze?: boolean;
  add_step?: string; set_step?: [number, boolean]; rename_step?: [number, string]; remove_step?: number;
  add_requirement?: string; set_requirement?: [number, boolean]; remove_requirement?: number;
}

export const api = {
  brief: () => invoke<Brief>("brief"),
  listItems: (status: string, kind: string, query: string) =>
    invoke<Item[]>("list_items", { status, kind: kind || null, query: query || null }),
  getItem: (id: string) => invoke<Item>("get_item", { id }),
  previewQuickAdd: (text: string) =>
    invoke<{ title: string; due_label: string; due_local: string; all_day: boolean }>("preview_quick_add", { text }),
  previewDue: (text: string) => invoke<{ due_label: string; due_local: string; all_day: boolean }>("preview_due", { text }),
  quickAdd: (text: string, kind: Kind | null) => invoke<Item>("quick_add", { text, kind }),
  updateItem: (id: string, changes: Changes) => invoke<Item>("update_item", { id, changes }),
  completeItem: (id: string) => invoke<{ item: Item; next: Item | null }>("complete_item", { id }),
  snoozeItem: (id: string, until: string) => invoke<Item>("snooze_item", { id, until }),
  deleteItem: (id: string) => invoke<Item>("delete_item", { id }),
  restoreItem: (id: string) => invoke<Item>("restore_item", { id }),
  getRoutine: () => invoke<RoutineBlock[]>("get_routine"),
  setRoutine: (blocks: RoutineBlock[]) => invoke<RoutineBlock[]>("set_routine", { blocks }),
  getSettings: () => invoke<SettingsInfo>("get_settings"),
  saveSettings: (settings: Settings) => invoke<SettingsInfo>("save_settings", { settings }),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  planWeek: () => invoke<WeekPlan>("plan_week"),
  exportJson: () => invoke<string | null>("export_json"),
  importJson: () => invoke<number | null>("import_json"),
  exportIcs: () => invoke<string | null>("export_ics"),
  connectInfo: () => invoke<ConnectInfo>("connect_info"),
  connectClaudeDesktop: () => invoke<ConnectOutcome>("connect_claude_desktop"),
  testNotification: () => invoke<void>("test_notification"),
};

/** "17:00" from an ISO string with offset, without letting the browser's timezone interfere. */
export function localTime(iso: string): string {
  return iso.slice(11, 16);
}

export function errorText(e: unknown): string {
  return typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e);
}

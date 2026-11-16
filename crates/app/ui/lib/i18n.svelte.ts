import { en, ja, type Key } from "./strings";

const tables = { en, ja } as const;
export type Lang = keyof typeof tables;

export const i18n = $state({ lang: "en" as Lang });

export function t(key: Key, vars: Record<string, string | number> = {}): string {
  const raw = (tables[i18n.lang] as Record<Key, string>)[key] || en[key];
  return raw.replace(/\{(\w+)\}/g, (_, name: string) => String(vars[name] ?? `{${name}}`));
}

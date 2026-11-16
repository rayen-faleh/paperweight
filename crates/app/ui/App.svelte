<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { api } from "./lib/api";
  import { i18n, t } from "./lib/i18n.svelte";
  import { ui, changed, prettyShortcut, type View } from "./lib/state.svelte";
  import Today from "./views/Today.svelte";
  import AllItems from "./views/AllItems.svelte";
  import Routine from "./views/Routine.svelte";
  import Archive from "./views/Archive.svelte";
  import SettingsView from "./views/Settings.svelte";
  import ItemDetail from "./views/ItemDetail.svelte";
  import QuickAdd from "./views/QuickAdd.svelte";

  const nav: { view: View; key: "nav.today" | "nav.all" | "nav.routine" | "nav.archive" | "nav.settings" }[] = [
    { view: "today", key: "nav.today" },
    { view: "all", key: "nav.all" },
    { view: "routine", key: "nav.routine" },
    { view: "archive", key: "nav.archive" },
    { view: "settings", key: "nav.settings" },
  ];

  async function loadSettings() {
    try {
      const s = await api.getSettings();
      i18n.lang = s.settings.language;
      ui.shortcut = s.settings.quick_add_shortcut;
    } catch {
      /* the views show their own errors */
    }
  }

  onMount(() => {
    loadSettings();
    // Allow a view to be chosen by URL hash (used for screenshots): #today, #settings, #item=pw-xxxx.
    const hash = location.hash.slice(1);
    if (hash.startsWith("item=")) ui.selected = hash.slice(5);
    else if (["today", "all", "routine", "archive", "settings"].includes(hash)) ui.view = hash as View;
    else if (hash === "quick") ui.quickAdd = true;

    const unlisten = [
      listen("data-changed", () => {
        changed();
        loadSettings();
      }),
      listen<string>("open-item", (e) => {
        ui.quickAdd = false;
        ui.selected = e.payload;
      }),
      listen("quick-add", () => {
        ui.selected = null;
        ui.quickAdd = true;
      }),
    ];
    const onKey = (e: KeyboardEvent) => {
      const typing = e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement;
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === "k") {
        e.preventDefault();
        ui.quickAdd = true;
      } else if (!typing && e.key === "n" && !e.ctrlKey && !e.metaKey && !e.altKey) {
        e.preventDefault();
        ui.quickAdd = true;
      } else if (e.key === "Escape") {
        if (ui.quickAdd) ui.quickAdd = false;
        else if (ui.selected) ui.selected = null;
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      unlisten.forEach((p) => p.then((f) => f()));
      window.removeEventListener("keydown", onKey);
    };
  });
</script>

<div class="shell">
  <aside class="rail">
    <div class="brand">
      <svg viewBox="0 0 64 64" width="22" height="22" aria-hidden="true">
        <path d="M6 52 A26 26 0 0 1 58 52 Z" fill="var(--accent)" />
        <rect x="4" y="53" width="56" height="5" rx="1" fill="var(--accent)" />
      </svg>
      <span>{t("app.name")}</span>
    </div>
    <nav>
      {#each nav as n (n.view)}
        <button
          class="nav"
          class:active={ui.view === n.view}
          onclick={() => {
            ui.view = n.view;
            ui.selected = null;
          }}>{t(n.key)}</button
        >
      {/each}
    </nav>
    <button class="add" onclick={() => (ui.quickAdd = true)}>
      <span>+ {t("nav.quickAdd")}</span>
      {#if ui.shortcut}<kbd>{prettyShortcut(ui.shortcut)}</kbd>{/if}
    </button>
  </aside>

  <main>
    {#key ui.view}
      {#if ui.view === "today"}<Today />
      {:else if ui.view === "all"}<AllItems />
      {:else if ui.view === "routine"}<Routine />
      {:else if ui.view === "archive"}<Archive />
      {:else}<SettingsView />
      {/if}
    {/key}
  </main>

  {#if ui.selected}
    <ItemDetail id={ui.selected} />
  {/if}
  {#if ui.quickAdd}
    <QuickAdd />
  {/if}
  {#if ui.toast}
    <div class="toast" role="status">{ui.toast.text}</div>
  {/if}
</div>

<style>
  .shell {
    display: grid;
    grid-template-columns: 210px 1fr;
    height: 100%;
  }
  .rail {
    display: flex;
    flex-direction: column;
    gap: 22px;
    padding: 22px 14px;
    border-right: 1px solid var(--line);
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 10px;
    font-weight: 650;
    letter-spacing: -0.01em;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .nav {
    text-align: left;
    border: 0;
    background: transparent;
    padding: 8px 10px;
    color: var(--ink-2);
  }
  .nav:hover {
    background: var(--sunken);
  }
  .nav.active {
    background: var(--surface);
    color: var(--ink);
    font-weight: 600;
    box-shadow: inset 2px 0 0 var(--accent);
  }
  .add {
    margin-top: auto;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 4px;
    padding: 10px 12px;
    color: var(--ink-2);
    text-align: left;
  }
  .add span {
    white-space: nowrap;
  }
  kbd {
    font-family: var(--font);
    font-size: 11px;
    color: var(--muted);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 1px 5px;
  }
  main {
    overflow-y: auto;
    padding: 40px 48px 80px;
  }
  .toast {
    position: fixed;
    left: 50%;
    bottom: 26px;
    transform: translateX(-50%);
    background: var(--ink);
    color: var(--bg);
    padding: 10px 16px;
    border-radius: 9px;
    box-shadow: var(--shadow);
    font-size: 14px;
    max-width: min(560px, 90vw);
    z-index: 50;
  }
  @media (max-width: 720px) {
    .shell {
      grid-template-columns: 1fr;
      grid-template-rows: auto 1fr;
    }
    .rail {
      flex-direction: row;
      align-items: center;
      gap: 8px;
      padding: 10px 12px;
      border-right: 0;
      border-bottom: 1px solid var(--line);
      overflow-x: auto;
    }
    .brand span {
      display: none;
    }
    nav {
      flex-direction: row;
    }
    .add {
      margin: 0 0 0 auto;
    }
    .add kbd {
      display: none;
    }
    main {
      padding: 24px 18px 80px;
    }
  }
</style>

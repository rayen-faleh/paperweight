<script lang="ts">
  import { api, errorText, type ConnectInfo, type ConnectOutcome, type SettingsInfo, type Settings } from "../lib/api";
  import { i18n, t } from "../lib/i18n.svelte";
  import { ui, toast, prettyShortcut } from "../lib/state.svelte";

  let info = $state<SettingsInfo | null>(null);
  let s = $state<Settings | null>(null);
  let connect = $state<ConnectInfo | null>(null);
  let outcome = $state<ConnectOutcome | null>(null);
  let error = $state<string | null>(null);
  let newOffset = $state("");
  let copied = $state<string | null>(null);

  const presets = ["7d", "2d", "day", "overdue"] as const;

  async function load() {
    try {
      info = await api.getSettings();
      s = $state.snapshot(info.settings);
      connect = await api.connectInfo();
      error = null;
    } catch (e) {
      error = errorText(e);
    }
  }
  load();

  async function save() {
    if (!s) return;
    try {
      info = await api.saveSettings(s);
      s = $state.snapshot(info.settings);
      i18n.lang = s.language;
      ui.shortcut = s.quick_add_shortcut;
      toast(t("settings.saved"));
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }

  function toggleOffset(o: string) {
    if (!s) return;
    s.reminder_offsets = s.reminder_offsets.includes(o) ? s.reminder_offsets.filter((x) => x !== o) : [...s.reminder_offsets, o];
  }

  function addOffset(e: Event) {
    e.preventDefault();
    const o = newOffset.trim().toLowerCase();
    if (s && o && !s.reminder_offsets.includes(o)) s.reminder_offsets = [...s.reminder_offsets, o];
    newOffset = "";
  }

  async function copy(text: string, key: string) {
    await navigator.clipboard.writeText(text);
    copied = key;
    setTimeout(() => (copied = null), 1500);
  }

  async function run<T>(f: () => Promise<T>, done: (v: T) => void) {
    try {
      done(await f());
    } catch (e) {
      toast(t("common.error", { message: errorText(e) }));
    }
  }

  const offsetLabel = (o: string) =>
    (presets as readonly string[]).includes(o) ? t(`settings.offset.${o as (typeof presets)[number]}`) : o;
  const dirty = $derived(info && s ? JSON.stringify(info.settings) !== JSON.stringify(s) : false);
</script>

<section class="page">
  <div class="head">
    <h1>{t("settings.title")}</h1>
    <button class="primary" disabled={!dirty} onclick={save}>{t("settings.save")}</button>
  </div>
  {#if error}<p class="muted">{t("common.error", { message: error })}</p>{/if}

  {#if s && info}
    <div class="card">
      <h2>{t("settings.reminders")}</h2>
      <div class="row">
        <div class="lab">{t("settings.quiet")}<p class="muted small">{t("settings.quietHint")}</p></div>
        <div class="inline">
          <span class="muted small">{t("settings.from")}</span><input type="time" bind:value={s.quiet_start} />
          <span class="muted small">{t("settings.to")}</span><input type="time" bind:value={s.quiet_end} />
        </div>
      </div>
      <div class="row">
        <div class="lab">{t("settings.offsets")}<p class="muted small">{t("settings.offsetsHint")}</p></div>
        <div class="chips">
          {#each [...new Set([...presets, ...s.reminder_offsets])] as o (o)}
            <button class="chip" class:on={s.reminder_offsets.includes(o)} onclick={() => toggleOffset(o)}>{offsetLabel(o)}</button>
          {/each}
          <form onsubmit={addOffset}><input class="tiny" placeholder={t("settings.addOffset")} bind:value={newOffset} /></form>
        </div>
      </div>
      <div class="row">
        <div class="lab">{t("settings.rollover")}<p class="muted small">{t("settings.rolloverHint")}</p></div>
        <input type="time" class="fit" bind:value={s.day_rollover} />
      </div>
      <div class="row">
        <div class="lab">{t("settings.timezone")}<p class="muted small">{t("settings.timezoneHint", { tz: info.effective_timezone })}</p></div>
        <input class="fit" bind:value={s.timezone} />
      </div>
      <div class="row">
        <div></div>
        <button onclick={() => run(api.testNotification, () => toast(t("settings.testSent")))}>{t("settings.testNotification")}</button>
      </div>
    </div>

    <div class="card" id="connect">
      <h2>{t("connect.title")}</h2>
      <p class="muted">{t("connect.intro")}</p>
      {#if connect}
        {#if connect.binary}
          <div class="kv"><span class="label">{t("connect.binary")}</span><code class="mono">{connect.binary}</code></div>
          <h3>{t("connect.any")}</h3>
          <p class="muted small">{t("connect.anyHint")}</p>
          <div class="code">
            <pre class="mono">{connect.snippet}</pre>
            <button class="ghost small" onclick={() => copy(connect!.snippet ?? "", "snippet")}>{copied === "snippet" ? t("settings.copied") : t("settings.copy")}</button>
          </div>
          <h3>{t("connect.claude")}</h3>
          <p class="muted small">{t("connect.claudeHint")}</p>
          {#if connect.claude_config_path}<p class="small mono path">{t("connect.claudeConfig", { path: connect.claude_config_path })}</p>{/if}
          {#if !connect.claude_config_exists}<p class="small">{t("connect.claudeMissing")}</p>{/if}
          <button class="primary" onclick={() => run(api.connectClaudeDesktop, (o) => (outcome = o))}>{t("connect.claudeButton")}</button>
          {#if outcome?.outcome === "written"}
            <p class="ok">{t("connect.written", { backup: outcome.backup })}</p>
            <p class="small muted">{t("connect.result")}</p>
            <pre class="mono result">{outcome.result}</pre>
          {:else if outcome?.outcome === "manual"}
            <p class="warn">{t("connect.manual", { reason: outcome.reason })}</p>
            <ol class="steps small">
              {#each outcome.steps as step, i (i)}<li>{#if step.trim().startsWith("{")}<pre class="mono">{step}</pre>{:else}{step}{/if}</li>{/each}
            </ol>
          {/if}
        {:else}
          <p class="warn">{t("connect.notFound")}</p>
        {/if}
      {/if}
    </div>

    <div class="card">
      <h2>{t("settings.calendar")}</h2>
      <div class="row">
        <div class="lab">{t("settings.feed")}<p class="muted small">{t("settings.feedHint")}</p></div>
        <div class="stack">
          <div class="code inline-code"><code class="mono">{info.feed_url}</code>
            <button class="ghost small" onclick={() => copy(info!.feed_url, "feed")}>{copied === "feed" ? t("settings.copied") : t("settings.copy")}</button></div>
          {#if info.feed_error}<p class="warn small">{info.feed_error}</p>{/if}
        </div>
      </div>
      <div class="row">
        <div class="lab">{t("settings.port")}</div>
        <input type="number" class="fit" min="1024" max="65535" bind:value={s.feed_port} />
      </div>
      <div class="row">
        <div></div>
        <button onclick={() => run(api.exportIcs, (p) => p && toast(t("settings.exported", { path: p })))}>{t("settings.exportIcs")}</button>
      </div>
    </div>

    <div class="card">
      <h2>{t("settings.general")}</h2>
      <div class="row">
        <div class="lab">{t("settings.language")}</div>
        <select class="fit" bind:value={s.language}>
          <option value="en">English</option>
          <option value="ja">日本語</option>
        </select>
      </div>
      <div class="row">
        <div class="lab">{t("settings.shortcut")}<p class="muted small">{t("settings.shortcutHint")} ({prettyShortcut(s.quick_add_shortcut)})</p></div>
        <div class="stack">
          <input class="fit mono" bind:value={s.quick_add_shortcut} />
          {#if info.shortcut_error}<p class="warn small">{info.shortcut_error}</p>{/if}
        </div>
      </div>
      <div class="row">
        <label class="lab inline" for="autostart">{t("settings.autostart")}</label>
        <input id="autostart" class="check" type="checkbox" checked={info.autostart}
          onchange={(e) => run(() => api.setAutostart((e.currentTarget as HTMLInputElement).checked), (v) => info && (info.autostart = v))} />
      </div>
    </div>

    <div class="card">
      <h2>{t("settings.data")}</h2>
      <p class="muted small">{t("settings.dataHint")}</p>
      <div class="kv"><span class="label">{t("settings.location")}</span><code class="mono">{info.db_path}</code></div>
      <div class="kv"><span class="label">{t("settings.log")}</span><code class="mono">{info.log_path}</code></div>
      <div class="buttons">
        <button onclick={() => run(api.exportJson, (p) => p && toast(t("settings.exported", { path: p })))}>{t("settings.exportJson")}</button>
        <button onclick={() => run(api.importJson, (n) => n !== null && (toast(t("settings.imported", { n })), load()))}>{t("settings.importJson")}</button>
      </div>
    </div>
  {/if}
</section>

<style>
  .page {
    max-width: 780px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 18px;
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    position: sticky;
    top: -40px;
    padding: 12px 0;
    background: var(--bg);
    z-index: 2;
  }
  h1 {
    font-size: 26px;
  }
  .card {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 12px;
    padding: 22px 24px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .card h2 {
    font-size: 17px;
  }
  .card h3 {
    font-size: 14px;
    margin-top: 6px;
  }
  .card > p {
    margin: -6px 0 0;
  }
  .row {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 20px;
    align-items: center;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }
  .lab p {
    margin: 2px 0 0;
    max-width: 380px;
  }
  .inline {
    display: flex;
    gap: 8px;
    align-items: center;
  }
  .inline input {
    width: 120px;
  }
  .fit {
    width: 220px !important;
  }
  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    justify-content: flex-end;
    max-width: 360px;
  }
  .chip {
    font-size: 13px;
    padding: 4px 11px;
    border-radius: 99px;
    color: var(--muted);
  }
  .chip.on {
    border-color: var(--accent);
    color: var(--accent);
    background: var(--accent-soft);
  }
  .tiny {
    width: 120px !important;
    padding: 4px 9px !important;
    font-size: 13px;
  }
  .stack {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
  }
  .stack p {
    margin: 0;
  }
  .kv {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .kv code,
  .path {
    overflow-wrap: anywhere;
  }
  .code {
    position: relative;
    background: var(--sunken);
    border-radius: 8px;
  }
  .code pre {
    margin: 0;
    padding: 12px 14px;
    overflow-x: auto;
  }
  .code button {
    position: absolute;
    top: 6px;
    right: 6px;
  }
  .inline-code {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 4px 4px 12px;
  }
  .inline-code button {
    position: static;
  }
  .result {
    background: var(--sunken);
    padding: 12px;
    border-radius: 8px;
    max-height: 260px;
    overflow: auto;
    margin: 0;
  }
  .steps pre {
    background: var(--sunken);
    padding: 10px;
    border-radius: 8px;
  }
  .buttons {
    display: flex;
    gap: 8px;
  }
  .card > button {
    align-self: flex-start;
  }
  .ok {
    color: var(--ink-2);
  }
  .warn {
    color: var(--accent);
  }
  @media (max-width: 720px) {
    .row {
      grid-template-columns: 1fr;
      gap: 8px;
    }
    .chips,
    .stack {
      justify-content: flex-start;
      align-items: flex-start;
    }
  }
</style>

<script lang="ts">
  import { appStore } from "../lib/ssot";
  import { setWatchPaths } from "../lib/ipc";
  import { openDirectory } from "../lib/dialog";
  import { invoke } from "../lib/tauri-utils";

  let form = $state<{
    theme: string;
    auto_reindex: string;
    default_budget: number;
    default_lambda: number;
    watch_paths: string[];
  }>({
    theme: "System",
    auto_reindex: "Off",
    default_budget: 2000,
    default_lambda: 0.7,
    watch_paths: [],
  });

  async function load() {
    try {
      const s: any = await invoke("get_settings");
      form = { ...form, ...s, watch_paths: s.watch_paths || [] };
    } catch {}
  }

  function close() {
    appStore.update((s) => ({ ...s, settingsOpen: false }));
  }

  function applyTheme(theme: string) {
    if (theme === "Light")
      document.body.classList.add("theme-light"),
        document.body.classList.remove("theme-dark");
    else if (theme === "Dark")
      document.body.classList.add("theme-dark"),
        document.body.classList.remove("theme-light");
    else {
      document.body.classList.remove("theme-light", "theme-dark");
      const prefersDark =
        window.matchMedia &&
        window.matchMedia("(prefers-color-scheme: dark)").matches;
      document.body.classList.add(prefersDark ? "theme-dark" : "theme-light");
    }
  }

  async function save() {
    try {
      appStore.update((s) => ({
        ...s,
        theme: form.theme.toLowerCase() as any,
      }));
      applyTheme(form.theme);

      // Persist watch paths and other settings (normalize paths from backend response)
      const normalized = await setWatchPaths(form.watch_paths);
      form.watch_paths = normalized;
      await invoke("set_settings", { partial: form });
      close();
    } catch (e) {
      appStore.update((s) => ({
        ...s,
        errorObj: { code: "E_SETTINGS", message: String(e) },
      }));
    }
  }

  function removePath(index: number) {
    form.watch_paths = form.watch_paths.filter((_, idx) => idx !== index);
  }

  async function addFolder() {
    try {
      const dir = await openDirectory();
      if (dir && dir.length > 0) {
        if (!form.watch_paths.includes(dir)) {
          form.watch_paths = [...form.watch_paths, dir];
        }
      }
    } catch (e) {
      appStore.update((s) => ({
        ...s,
        errorObj: { code: "E_DIALOG", message: String(e) },
      }));
    }
  }

  $effect(() => {
    if ($appStore.settingsOpen) load();
  });
</script>

{#if $appStore.settingsOpen}
  <button
    style="position:fixed; inset:0; background:rgba(0,0,0,.35); border:none; cursor:pointer;"
    onclick={close}
    aria-label="Close settings"
  ></button>
  <div
    style="position:fixed; right:0; top:0; width:420px; height:100%; background:var(--bg-panel,#f1f3f4); backdrop-filter: blur(8px); padding:12px; border-left: 1px solid var(--border-color, rgba(0,0,0,.1))"
  >
    <div
      style="display:flex; justify-content:space-between; align-items:center"
    >
      <strong>Settings</strong>
      <button onclick={close}>✕</button>
    </div>
    <div style="margin-top:12px">
      <div>
        <label for="theme-select">Theme</label><br />
        <select
          id="theme-select"
          bind:value={form.theme}
          onchange={() => applyTheme(form.theme)}
        >
          <option>System</option>
          <option>Light</option>
          <option>Dark</option>
        </select>
      </div>
      <div style="margin-top:8px">
        <label for="reindex-select">Auto Reindex</label><br />
        <select id="reindex-select" bind:value={form.auto_reindex}>
          <option>Off</option>
          <option>On</option>
        </select>
      </div>
      <div style="margin-top:8px">
        <label for="budget-input">Default Budget</label><br />
        <input
          id="budget-input"
          type="number"
          bind:value={form.default_budget}
        />
      </div>
      <div style="margin-top:8px">
        <label for="lambda-input">Default MMR λ</label><br />
        <input
          id="lambda-input"
          type="number"
          step="0.05"
          bind:value={form.default_lambda}
        />
      </div>
      <div style="margin-top:8px">
        <h3 style="margin:0 0 4px 0; font-size:14px;">Watch Folders</h3>
        <div style="font-size:11px; color:#999; margin-bottom:6px">
          Paths are automatically normalized and sorted by the backend
        </div>
        <div>
          {#each form.watch_paths as path, i (path)}
            <div
              style="display:flex; align-items:center; gap:6px; margin:4px 0"
            >
              <code style="flex:1">{path}</code>
              <button onclick={() => removePath(i)}>Remove</button>
            </div>
          {/each}
          <div style="margin-top:6px">
            <button onclick={addFolder}>Add Folder…</button>
          </div>
        </div>
      </div>
    </div>
    <div style="position:absolute; bottom:12px; right:12px">
      <button onclick={save}>Save</button>
    </div>
  </div>
{/if}

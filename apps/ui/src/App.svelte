<script lang="ts">
  import TopBar from './components/TopBar.svelte';
  import DocumentsPanel from './components/DocumentsPanel.svelte';
  import Viewer from './components/Viewer.svelte';
  import RightDockPrompt from './components/RightDockPrompt.svelte';
  import RightDockOntology from './components/RightDockOntology.svelte';
  import SettingsDrawer from './components/SettingsDrawer.svelte';
  import ErrorBanner from './components/ErrorBanner.svelte';
  import { appStore } from './lib/ssot';
  import { safeInvoke, safeListen, isTauriEnvironment } from './lib/tauri-utils';
  import { onMount } from 'svelte';
  import { get } from 'svelte/store';
  import { listDocuments, readDocument, selectWorkspace } from './lib/ipc';


  // Global shortcuts with proper typing
  function onKeyDown(ev: KeyboardEvent) {
    if ((ev.metaKey || ev.ctrlKey) && ev.key.toLowerCase() === 'o') {
      ev.preventDefault();
      const btn = document.querySelector('.topbar button') as HTMLButtonElement;
      btn?.click();
    }
    if (ev.key.toLowerCase() === 'p') {
      appStore.update(s => ({ ...s, rightDockMode: s.rightDockMode === 'prompt' ? 'ontology' : 'prompt' }));
    }
    if (ev.key.toLowerCase() === 'e') {
      appStore.update(s => ({ ...s, editMode: !s.editMode }));
    }
    if (ev.key.toLowerCase() === 'j') {
      appStore.update(s => ({ ...s, showJsonMeta: !s.showJsonMeta }));
    }
  }

  function applyTheme(theme: string) {
    document.body.classList.remove('theme-light', 'theme-dark');
    if (theme.toLowerCase() === 'light') {
      document.body.classList.add('theme-light');
    } else if (theme.toLowerCase() === 'dark') {
      document.body.classList.add('theme-dark');
    } else {
      // System theme
      const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
      document.body.classList.add(prefersDark ? 'theme-dark' : 'theme-light');
    }
  }

  onMount(async () => {
    try {
      // Check if running in Tauri environment
      if (!isTauriEnvironment()) {
        console.warn('Running in browser mode - Tauri features disabled');
        applyTheme('system');
        return;
      }

      // Set default workspace to documents folder relative to current directory
      try {
        const defaultWorkspacePath = './documents';
        await selectWorkspace(defaultWorkspacePath);
        const docs = await listDocuments();
        console.log('Documents loaded:', docs);
        appStore.update(s => ({ ...s, documents: docs, workspacePath: defaultWorkspacePath }));
      } catch (e) {
        console.warn('Failed to set default workspace:', e);
        // Try alternative paths
        try {
          const altPath = 'documents';
          await selectWorkspace(altPath);
          const docs = await listDocuments();
          console.log('Documents loaded (alt path):', docs);
          appStore.update(s => ({ ...s, documents: docs, workspacePath: altPath }));
        } catch (e2) {
          console.warn('Failed to set alternative workspace:', e2);
        }
      }

      // Apply persisted theme at startup
      try {
        const settings = await safeInvoke<{ theme?: string }>('get_settings');
        if (settings?.theme) {
          applyTheme(settings.theme);
          appStore.update(s => ({ ...s, theme: settings.theme as any }));
        } else {
          applyTheme('system');
        }
      } catch (e) {
        console.warn('Failed to load settings:', e);
        applyTheme('system');
      }

      // Listen for workspace changes
      const unlisten = await safeListen('workspace://change', async (event) => {
        try {
          const docs = await listDocuments();
          appStore.update(s => ({ ...s, documents: docs }));
          
          const paths = (event.payload as any)?.paths as string[] | undefined;
          const currentDoc = get(appStore).selectedDoc;
          if (paths && currentDoc) {
            const needsRefresh = paths.some(p => p.endsWith(currentDoc.path));
            if (needsRefresh) {
              try {
                const view = await readDocument(currentDoc.id);
                appStore.update(s => ({ ...s, docView: view }));
              } catch (idError) {
                const view = await readDocument(currentDoc.path);
                appStore.update(s => ({ ...s, docView: view }));
              }
            }
          }
        } catch (e) {
          appStore.update(s => ({ ...s, errorObj: { code: 'E_WORKSPACE', message: String(e) } }));
        }
      });

      // Store cleanup function for later use
      if (unlisten) {
        const cleanup = unlisten;
        return () => cleanup();
      }
    } catch (e) {
      appStore.update(s => ({ ...s, errorObj: { code: 'E_INIT', message: String(e) } }));
    }
  });
</script>

<svelte:window onkeydown={onKeyDown} />

<main class="app">
  <TopBar />
  <ErrorBanner />
  <div class="columns">
    <DocumentsPanel />
    <Viewer />
    {#if $appStore.rightDockMode === 'prompt'}
      <RightDockPrompt />
    {:else}
      <RightDockOntology />
    {/if}
  </div>
  <SettingsDrawer />
</main>

<script lang="ts">
  import { appStore } from '../lib/ssot';
  import { selectWorkspace, listDocuments } from '../lib/ipc';
  import { openDirectory } from '../lib/dialog';

  let search = $state('');

  async function changeFolder() {
    try {
      const path = await openDirectory();
      
      if (!path) return;

      appStore.update(s => ({ ...s, busy: true, errorObj: null }));
      
      const ws = await selectWorkspace(path);
      const docs = await listDocuments();
      
      appStore.update(s => ({ ...s, 
        workspaceRoot: ws.root, 
        documents: docs, 
        busy: false 
      }));
    } catch (e) {
      appStore.update(s => ({ ...s, 
        busy: false, 
        errorObj: { code: 'E_WORKSPACE', message: String(e) } 
      }));
    }
  }

  function toggleDockMode() {
    appStore.update(s => ({ ...s, 
      rightDockMode: s.rightDockMode === 'prompt' ? 'ontology' : 'prompt' 
    }));
  }

  function openSettings() {
    appStore.update(s => ({ ...s, settingsOpen: true }));
  }
</script>

<div class="topbar">
  <div>
    <button onclick={changeFolder}>← Folder</button>
    <span style="margin-left:8px;opacity:.8">{$appStore.workspaceRoot ?? '—'}</span>
  </div>
  <div>
    <input 
      placeholder="Search (⌘/Ctrl+K)" 
      bind:value={search} 
      style="width:100%" 
    />
  </div>
  <div style="text-align:right">
    <button onclick={toggleDockMode}>
      Prompt ⇄ Ontology
    </button>
    <button style="margin-left:6px" onclick={openSettings}>⚙︎</button>
  </div>
  {#if $appStore.busy}
    <div style="grid-column: 1 / -1; height:2px; background:linear-gradient(90deg, #09f, transparent)"></div>
  {/if}
</div>

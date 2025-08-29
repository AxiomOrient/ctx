<script lang="ts">
  import { appStore } from '../lib/ssot';
  import { updateDocument, readDocument } from '../lib/ipc';
  import { get } from 'svelte/store';

  let draft = $state('');
  
  // Reactive assignment using store subscription
  $effect(() => {
    if ($appStore.docView && $appStore.editMode) {
      draft = $appStore.docView.raw_markdown;
    }
  });

  function toggleEdit() {
    appStore.update(s => ({ ...s, editMode: !s.editMode }));
  }

  async function save() {
    const currentDoc = get(appStore).selectedDoc;
    if (!currentDoc) return;
    
    try {
      appStore.update(s => ({ ...s, busy: true, errorObj: null }));
      
      await updateDocument(currentDoc.path, { 
        Full: { content: draft } 
      });
      
      const view = await readDocument(currentDoc.path);
      appStore.update(s => ({ ...s, 
        docView: view, 
        busy: false, 
        editMode: false 
      }));
    } catch (e) {
      appStore.update(s => ({ ...s, 
        busy: false, 
        errorObj: { code: 'E_DOC_SAVE', message: String(e) } 
      }));
    }
  }
</script>

<div class="panel" style="padding:12px">
    <div style="margin-bottom:8px">
        <button onclick={toggleEdit}>
            {$appStore.editMode ? "View" : "Edit"}
        </button>
        {#if $appStore.editMode}
            <button
                onclick={save}
                style="margin-left:6px"
                disabled={$appStore.busy}
            >
                {$appStore.busy ? "Saving..." : "Save"}
            </button>
        {/if}
    </div>
    {#if $appStore.docView}
        {#if $appStore.editMode}
            <textarea style="width:100%; height:60vh" bind:value={draft}
            ></textarea>
        {:else}
            <!-- Backend provides sanitized HTML. -->
            <div class="md">{@html $appStore.docView.html}</div>
        {/if}
    {:else}
        <div style="opacity:.6">Select a document to preview…</div>
    {/if}
</div>

<script lang="ts">
    import { appStore } from "../lib/ssot";
    import { readDocument } from "../lib/ipc";
    import type { DocMeta } from "../lib/types";

    async function openDoc(doc: DocMeta) {
        try {
            console.log("Opening document:", doc);
            appStore.update((s) => ({
                ...s,
                selectedDoc: doc,
                errorObj: null,
            }));

            // Try with id first, then path if that fails
            let view;
            try {
                console.log("Trying to read document with id:", doc.id);
                view = await readDocument(doc.id);
            } catch (idError) {
                console.warn(
                    "Failed to read document with id, trying path:",
                    doc.path,
                    idError,
                );
                view = await readDocument(doc.path);
            }

            appStore.update((s) => ({ ...s, docView: view }));
        } catch (e) {
            console.error("Document read error:", e);
            const errorMessage = e instanceof Error ? e.message : String(e);
            appStore.update((s) => ({
                ...s,
                errorObj: { code: "E_DOC_READ", message: errorMessage },
            }));
        }
    }
</script>

<div class="panel">
    {#if $appStore.workspacePath}
        <div class="workspace-path">
            <strong>📁 {$appStore.workspacePath}</strong>
        </div>
    {/if}

    {#each $appStore.documents as doc (doc.id)}
        <button
            class="doc-item"
            class:selected={$appStore.selectedDoc?.id === doc.id}
            onclick={() => openDoc(doc)}
        >
            <div><strong>{doc.id}</strong></div>
            <div style="opacity:.7; font-size:12px">{doc.path}</div>
            <div>
                {#each doc.tags.slice(0, 4) as tag}
                    <span class="badge">{tag}</span>
                {/each}
            </div>
        </button>
    {/each}
</div>

<style>
    .workspace-path {
        padding: 8px 12px;
        background: var(--bg-secondary, #f8f9fa);
        border-bottom: 1px solid var(--border-color, #e1e5e9);
        font-size: 13px;
        color: var(--text-secondary, #6c757d);
    }
</style>

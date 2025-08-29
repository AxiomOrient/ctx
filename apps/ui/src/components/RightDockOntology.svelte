<script lang="ts">
    import { appStore } from "../lib/ssot";
    import { safeInvoke, isTauriEnvironment } from "../lib/tauri-utils";

    let tab = $state<"ontology" | "rules">("ontology");
    let text = $state("");

    async function load() {
        if (!isTauriEnvironment()) {
            console.warn('Ontology loading not available in browser mode');
            return;
        }
        
        try {
            const command = tab === "ontology" ? "read_ontology" : "read_rules";
            const result = await safeInvoke<string>(command);
            if (result !== null) {
                text = result;
            }
        } catch (e) {
            appStore.update((s) => ({
                ...s,
                errorObj: { code: "E_ONTOLOGY_LOAD", message: String(e) },
            }));
        }
    }

    async function save() {
        if (!isTauriEnvironment()) {
            console.warn('Ontology saving not available in browser mode');
            return;
        }
        
        try {
            const command =
                tab === "ontology" ? "update_ontology" : "update_rules";
            await safeInvoke(command, { content: text });
        } catch (e) {
            appStore.update((s) => ({
                ...s,
                errorObj: { code: "E_ONTOLOGY_SAVE", message: String(e) },
            }));
        }
    }

    function switchTab(newTab: "ontology" | "rules") {
        tab = newTab;
        load();
    }

    $effect(() => {
        if ($appStore.rightDockMode === "ontology") load();
    });
</script>

<div
    class="panel right"
    style="padding:12px; display:grid; grid-template-rows:auto 1fr auto; gap:8px"
>
    <div>
        <button
            onclick={() => switchTab("ontology")}
            class:active={tab === "ontology"}>Ontology</button
        >
        <button
            onclick={() => switchTab("rules")}
            style="margin-left:6px"
            class:active={tab === "rules"}>Rules</button
        >
    </div>
    <div>
        <textarea bind:value={text} style="width:100%; height:65vh"></textarea>
    </div>
    <div style="text-align:right">
        <button onclick={save}>Save</button>
    </div>
</div>

<style>
    .active {
        background: rgba(0, 0, 0, 0.1);
    }
</style>

<script lang="ts">
  import { appStore } from '../lib/ssot';
  import { compose } from '../lib/ipc';
  import type { ComposeRequest } from '../lib/types';
  import { get } from 'svelte/store';

  async function generate() {
    const query = get(appStore).promptInput.trim();
    if (!query) return;

    try {
      appStore.update(s => ({ ...s, busy: true, errorObj: null }));
      
      const request: ComposeRequest = { query };
      const output = await compose(request);
      
      appStore.update(s => ({ ...s, 
        promptOutput: output, 
        busy: false 
      }));
    } catch (e) {
      appStore.update(s => ({ ...s, 
        busy: false, 
        errorObj: { code: 'E_COMPOSE', message: String(e) } 
      }));
    }
  }

  function onKeyDown(ev: KeyboardEvent) {
    if ((ev.metaKey || ev.ctrlKey) && ev.key === 'Enter') {
      ev.preventDefault();
      generate();
    }
  }

  async function copyPrompt() {
    const promptText = get(appStore).promptOutput?.prompt;
    if (promptText) {
      try {
        await navigator.clipboard.writeText(promptText);
      } catch (e) {
        console.warn('Failed to copy to clipboard:', e);
      }
    }
  }

  function updatePromptInput(value: string) {
    appStore.update(s => ({ ...s, promptInput: value }));
  }

  function toggleJsonView() {
    appStore.update(s => ({ ...s, showJsonMeta: !s.showJsonMeta }));
  }
</script>

<div class="panel right">
  <div class="prompt">
    <div>
      <strong>Prompt</strong>
    </div>
    <div>
      <textarea 
        bind:value={$appStore.promptInput} 
        onkeydown={onKeyDown} 
        placeholder="원하는 기능을 입력하세요 (예: 'API 설계 초안')"
      ></textarea>
      <div style="margin-top:6px">
        <button onclick={generate} disabled={$appStore.busy}>
          {$appStore.busy ? 'Generating...' : 'Generate (⌘/Ctrl+Enter)'}
        </button>
        <label style="margin-left:10px">
          <input 
            type="checkbox" 
            bind:checked={$appStore.showJsonMeta} 
            onchange={toggleJsonView}
          /> 
          JSON
        </label>
      </div>
    </div>
    <div>
      {#if $appStore.promptOutput}
        <div style="margin:6px 0; display:flex; gap:6px">
          <button onclick={copyPrompt}>Copy</button>
        </div>
        {#if $appStore.showJsonMeta}
          <div class="output">{JSON.stringify($appStore.promptOutput, null, 2)}</div>
        {:else}
          <div class="output">{$appStore.promptOutput.prompt}</div>
        {/if}
        <div style="margin-top:8px; font-size:12px; opacity:.7">
          Sources: {$appStore.promptOutput.sources.length} · 
          Tokens: {$appStore.promptOutput.tokens} · 
          Confidence: {Math.round($appStore.promptOutput.confidence * 100)}%
        </div>
      {/if}
    </div>
  </div>
</div>

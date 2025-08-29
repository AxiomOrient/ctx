<script lang="ts">
  import { appStore } from '../lib/ssot';
  
  function close() { 
    appStore.update(s => ({ ...s, errorObj: null })); 
  }
  
  function toggleJson() { 
    appStore.update(s => ({ ...s, showErrorJson: !s.showErrorJson })); 
  }
</script>

{#if $appStore.errorObj}
  <div class="error-banner">
    <div>
      <strong>{$appStore.errorObj.code}</strong>
      <span style="margin-left:8px">{$appStore.errorObj.message}</span>
      <button style="margin-left:8px" onclick={toggleJson}>
        {$appStore.showErrorJson ? 'Hide' : 'Show'} JSON
      </button>
    </div>
    <button onclick={close}>✕</button>
  </div>
  {#if $appStore.showErrorJson}
    <pre class="error-details">{JSON.stringify($appStore.errorObj, null, 2)}</pre>
  {/if}
{/if}


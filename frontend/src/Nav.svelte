<script>
  import { emptyCfg, cfg2str } from './globals.ts';
  import { onMount } from 'svelte';
  let view = "";
  onMount(() => {
    let urlParams = new URLSearchParams(window.location.search);
    view = urlParams.get('view');
  });
</script>

<nav class="tabs">
  {#if view === "folder"}
    <a href="?view=folder&dir=/&page=0&cfg={cfg2str(emptyCfg)}" class="active">Folder</a>
  {:else}
    <a href="?view=folder&dir=/&page=0&cfg={cfg2str(emptyCfg)}">Folder</a>
  {/if}

  {#if view === "control"}
    <a href="?view=control" class="active">Control</a>
  {:else}
    <a href="?view=control">Control</a>
  {/if}
  
</nav>

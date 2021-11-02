<script>
  import { onMount } from 'svelte';
  let view = "";
  onMount(() => {
    let urlParams = new URLSearchParams(window.location.search);
    view = urlParams.get('view');
  });
</script>

<nav class="tabs">
  {#if view === "folder"}
    <a href="?view=folder&sort=name&reverse=false&dir=/" class="active">Folder</a>
  {:else}
    <a href="?view=folder&sort=name&reverse=false&dir=/">Folder</a>
  {/if}

  {#if view === "control"}
    <a href="?view=control" class="active">Control</a>
  {:else}
    <a href="?view=control">Control</a>
  {/if}
  
</nav>

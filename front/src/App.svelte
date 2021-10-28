<script lang="ts">
import { onMount } from 'svelte';
import Folder from './Folder.svelte';
import Media from './Media.svelte';
import Control from './Control.svelte';

let view = "";
onMount(() => {
    let urlParams = new URLSearchParams(window.location.search);
    view = urlParams.get('view');
    if (window.location.search === "") {
      let base = window.location.origin + window.location.pathname;
      window.location.replace(`${base}?view=folder&sort=name&reverse=false&dir=/`);
    }
});
</script>

<main>

{#if view === "folder"}
<Folder/>
{:else if view === "media"}
<Media/>
{:else if view === "control"}
<Control/>
{:else if view === null}
{:else}
Unknown
{/if}

</main>

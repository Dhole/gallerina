<script>
  import { FileType, serverUrl } from './globals.ts';
  import { onMount, beforeUpdate } from 'svelte';
  import Nav from './Nav.svelte';

  let queryDirSplit = [];
  let paramFileRows = [];
  let queryDir = "";
  let querySort = "";
  let queryReverse = false;

  let rowSize = 8;

  function reload() {
    let base = window.location.origin + window.location.pathname;
    window.location.replace(`${base}?view=folder&sort=${querySort}&reverse=${queryReverse}&dir=${queryDir}`);
  }

  onMount(async () => {
    let w = window.innerWidth;
    if (w >= 1200) {
      rowSize = 8;
    } else if (w >= 800) {
      rowSize = 5;
    } else {
      rowSize = 3;
    }
    // let param = window.location.hash.substr(1);
    let urlParams = new URLSearchParams(window.location.search);
    let dir = urlParams.get('dir');
    queryDir = dir;
    let sort = urlParams.get('sort');
    querySort = sort;
    let reverse = urlParams.get('reverse');
    queryReverse = reverse === "true" ? true : false;
    let dirSplit = decodeURIComponent(dir).split("/");
    for (let i = 0; i < dirSplit.length - 1; i++) {
      dirSplit[i] = `${dirSplit[i]}/`;
    }
    console.log(dirSplit);
    queryDirSplit = dirSplit;

    let folderUrl = `${serverUrl}/folder?sort=${querySort}&reverse=${queryReverse}&dir=${dir}`;
    if (dir !== "/") {
      folderUrl = folderUrl.replace(/\/$/, '');
    }
    const res = await fetch(folderUrl);
    let folder = await res.json();
    // console.log(folder);
    let fileRows = [[]];
    let row = 0;
    folder.folders.forEach((folder) => {
      fileRows[row].push({typ: FileType.Folder, name: folder.name, media: folder.media});
      if (fileRows[row].length === rowSize) {
	row += 1;
	fileRows.push([]);
      }
    })
    folder.media.forEach((media) => {
      fileRows[row].push({typ: FileType.Image, name: media.name});
      if (fileRows[row].length === rowSize) {
	row += 1;
	fileRows.push([]);
      }
    });
    for (let i = fileRows[row].length % rowSize; i < rowSize; i++) {
      fileRows[row].push(null);
    }
    paramFileRows = fileRows;
    // console.log(fileRows);
  });
</script>

<div class="container">
  <Nav/>

  <h1>
{#each queryDirSplit as elem, i}
  <a href="?view=folder&sort={querySort}&reverse={queryReverse}&dir={queryDirSplit.slice(0, i+1).join("")}">
    {elem}
  </a>
{:else}
  loading...
{/each}
  </h1>

  <div style="border: 0.1em solid; border-radius: 0.2em; margin: 1em 0.1em;" class="row">
    <div style="display: flex; margin: 0.4em 1em;" class="col">

    <div>
      <label style="margin-right: 0.6em;" class="vcenter">Sort by </label>
    </div>
    <div style="margin-right: 1em;">
      <select style="width: 8em;" class="vcenter" id="sort" bind:value={querySort} on:change={reload}>
	<option value="name">name</option>
	<option value="taken">taken</option>
	<option value="modified">modified</option>
      </select>
    </div>
    <div style="margin-right: 0.6em;">
      <label class="vcenter" for="reverse">reverse</label>
    </div>
    <div>
      <input style="top: 42%" class="vcenter" type="checkbox" id="reverse" bind:checked={queryReverse} on:change={reload}>
    </div>

  </div>
</div>

{#each paramFileRows as fileRow}
  <div style="aspect-ratio: {rowSize};" class="grow">
    {#each fileRow as file}
      <div class="gcol">
	<div class="square">
	  {#if file === null}
	  {:else if file.typ === FileType.Folder}
	    <!--<a class="thumb folder" href="?view=folder&sort={querySort}&reverse={queryReverse}&dir={queryDir}{file.name}/"
       style="background-size: cover; background-position: center center; background-image: url('{serverUrl}/thumb?path={queryDir}{file.name}/{file.img}');">-->
       <a class="thumb folder" href="?view=folder&sort={querySort}&reverse={queryReverse}&dir={queryDir}{file.name}/">
	 <div class="folderlabel">{file.name}</div>
	      {#if file.media !== null}
	      <img class="folderimg" loading="lazy" src="{serverUrl}/thumb?path={queryDir}{file.name}/{file.media}">
	      {/if}
	    </a>
	  {:else if file.typ === FileType.Image}
	    <!--<a href="{serverUrl}/src?dir={queryDir}{file.name}">-->
	    <a href="?view=media&sort={querySort}&reverse={queryReverse}&dir={queryDir}&name={file.name}">
	    <img class="thumb image" loading="lazy" src="{serverUrl}/thumb?path={queryDir}{file.name}">
	    </a>
	  {/if}
	</div>
      </div>
    {/each}
  </div>
{:else}
<p>loading...</p>
{/each}

</div>

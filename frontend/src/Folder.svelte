<script>
  import { FileType, serverUrl, apiUrl, uiUrl } from './globals.ts';
  import { onMount, beforeUpdate } from 'svelte';
  import Nav from './Nav.svelte';

  let queryDirSplit = [];
  let paramFileRows = [];
  let queryDir = "";
  let cleanDir = "";
  let querySort = "";
  let queryReverse = false;
  let queryRaw = false;

  let rowSize = 8;

  function reload() {
    let base = window.location.origin + window.location.pathname;
    window.location.replace(`${base}?view=folder&sort=${querySort}&reverse=${queryReverse}&raw=${queryRaw}&dir=${queryDir}`);
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
    cleanDir = queryDir === "/" ? "" : queryDir;
    let sort = urlParams.get('sort');
    querySort = sort;
    let reverse = urlParams.get('reverse');
    queryReverse = reverse === "true" ? true : false;
    let raw = urlParams.get('raw');
    queryRaw = raw === "true" ? true : false;
    let dirSplit = decodeURIComponent(dir).split("/");
    let _queryDirSplit = [];
    for (let i = 0; i < dirSplit.length; i++) {
      let elem = undefined;
      if (i === 0) {
	elem = { name: "/", dir: "/" };
      } else if (i == dirSplit.length - 1) {
	elem = { name: dirSplit[i], dir: dirSplit.join("/") };
      } else {
	elem = { name: `${dirSplit[i]}/`, dir: dirSplit.slice(0, i+1).join("/") };
      }
      _queryDirSplit.push(elem);
    }
    console.log(dirSplit);
    queryDirSplit = _queryDirSplit;

    let folderUrl = apiUrl('folder', {'sort':querySort, 'reverse': queryReverse, 'dir': dir});
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
{#each queryDirSplit as elem}
  <a href="{uiUrl({'view':'folder', 'sort':querySort, 'reverse':queryReverse, 'raw': queryRaw, 'dir': elem.dir})}">
    {elem.name}
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
    <div>
      <select style="width: 8em;" class="vcenter" id="sort" bind:value={querySort} on:change={reload}>
	<option value="name">name</option>
	<option value="taken">taken</option>
	<option value="modified">modified</option>
      </select>
    </div>
    <div style="margin-left: 1em; margin-right: 0.6em;">
      <label class="vcenter" for="reverse">reverse</label>
    </div>
    <div>
      <input style="top: 42%" class="vcenter" type="checkbox" id="reverse" bind:checked={queryReverse} on:change={reload}>
    </div>
    <div style="margin-left: 1em; margin-right: 0.6em">
      <label class="vcenter" for="raw">raw</label>
    </div>
    <div>
      <input style="top: 42%" class="vcenter" type="checkbox" id="raw" bind:checked={queryRaw}>
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
	    <a class="thumb folder"
	      href="{uiUrl({'view':'folder', 'sort':querySort, 'reverse':queryReverse, 'raw':queryRaw, 'dir':`${cleanDir}/${file.name}`})}">
	      <div class="folderlabel">{file.name}</div>
	      {#if file.media !== null}
		<img class="folderimg" loading="lazy"
		  src="{apiUrl('thumb', {'path':`${cleanDir}/${file.name}/${file.media}`})}">
	      {/if}
	    </a>
	  {:else if file.typ === FileType.Image}
	    {#if queryRaw}
	      <a href="{apiUrl(`src/${encodeURIComponent(file.name)}`, {'dir':`${cleanDir}/`})}">
	      <img class="thumb image" loading="lazy"
		src="{apiUrl('thumb', {'path':`${cleanDir}/${file.name}`})}">
	      </a>
	    {:else}
	      <a href="{uiUrl({'view':'media', 'sort':querySort, 'reverse':queryReverse, 'dir':queryDir, 'name':file.name})}">
	      <img class="thumb image" loading="lazy"
		src="{apiUrl('thumb', {'path':`${cleanDir}/${file.name}`})}">
	      </a>
	    {/if}
	  {/if}
	</div>
      </div>
    {/each}
  </div>
{:else}
<p>loading...</p>
{/each}

</div>

<script>
  import { FileType, serverUrl, apiUrl, uiUrl, emptyCfg, cfg2str, str2cfg, trimPrefix, shuffleArray } from './globals.ts';
  import { onMount, beforeUpdate } from 'svelte';
  import Nav from './Nav.svelte';

  let queryDirSplit = [];
  let queryDir = "";
  let cleanDir = "";
  let queryCfg = emptyCfg;
  let paramItems = [];

  function reload() {
    let base = window.location.origin + window.location.pathname;
    window.location.replace(`${base}?view=folder&dir=${queryDir}&cfg=${cfg2str(queryCfg)}`);
  }

  function updateUrl() {
    let base = window.location.pathname;
    history.replaceState(null, '', `${base}?view=folder&dir=${queryDir}&cfg=${cfg2str(queryCfg)}`);
  }

  function bumpRandom() {
    queryCfg.randSeed += 1;
    reload();
  }

  onMount(async () => {
    // let param = window.location.hash.substr(1);
    let urlParams = new URLSearchParams(window.location.search);
    let dir = urlParams.get('dir');
    queryDir = dir;
    cleanDir = queryDir === "/" ? "" : queryDir;
    let cfg = urlParams.get('cfg');
    cfg = cfg == null ? emptyCfg : str2cfg(cfg);
    queryCfg = cfg;
    updateUrl();
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
    queryDirSplit = _queryDirSplit;

    let items = [];
    if (cfg.recursive === false) {
      let folderUrl = apiUrl('folder', {'sort':cfg.sort, 'reverse': cfg.reverse, 'dir': dir});
      if (dir !== "/") {
	folderUrl = folderUrl.replace(/\/$/, '');
      }
      const res = await fetch(folderUrl);
      let folder = await res.json();
      folder.folders.forEach((folder) => {
	items.push({typ: FileType.Folder, name: folder.name, media: folder.media});
      })
      folder.media.forEach((media) => {
	items.push({typ: FileType.Image, name: media.name});
      });
    } else {
      let folderUrl = apiUrl('folderRecursive', {'dir': dir});
      if (dir !== "/") {
	folderUrl = folderUrl.replace(/\/$/, '');
      }
      const res = await fetch(folderUrl);
      let folder = await res.json();
      folder.media.forEach((media) => {
	let name = `${trimPrefix(media.dir, dir)}/${media.name}`;
	name = trimPrefix(name, "/");
	items.push({typ: FileType.Image, name: name});
      });
    }
    if (cfg.sort === "random") {
      shuffleArray(items, cfg.randSeed);
    }
    paramItems = items;
  });
</script>

<div class="container">
  <Nav/>

  <h1>
{#each queryDirSplit as elem}
  <a href="{uiUrl({'view':'folder', 'dir': elem.dir, 'cfg': cfg2str(queryCfg)})}">
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
      <select style="width: 8em;" class="vcenter" id="sort" bind:value={queryCfg.sort} on:change={reload}>
	<option value="name">name</option>
	<option value="taken">taken</option>
	<option value="modified">modified</option>
	<option value="random">random</option>
      </select>
    </div>
    {#if queryCfg.sort === "random"}
    <div style="margin-left: 1em; margin-right: 0.6em;">
      <a class="button primary" on:click={bumpRandom}>shuffle</a>
    </div>
    {/if}
    <div style="margin-left: 1em; margin-right: 0.4em;">
      <label class="vcenter" for="reverse">reverse</label>
    </div>
    <div>
      <input style="top: 42%" class="vcenter" type="checkbox" id="reverse" bind:checked={queryCfg.reverse} on:change={reload}>
    </div>
    <div style="margin-left: 1em; margin-right: 0.4em">
      <label class="vcenter" for="raw">raw</label>
    </div>
    <div>
      <input style="top: 42%" class="vcenter" type="checkbox" id="raw" bind:checked={queryCfg.raw} on:change={updateUrl}>
    </div>
    <div style="margin-left: 1em; margin-right: 0.4em">
      <label class="vcenter" for="recursive">recursive</label>
    </div>
    <div>
      <input style="top: 42%" class="vcenter" type="checkbox" id="recursive" bind:checked={queryCfg.recursive} on:change={reload}>
    </div>

  </div>
</div>

<div class="constrain">
<div class="gridContainer">
{#if paramItems === []}
  <p>loading...</p>
{:else}
  {#each paramItems as item}
    <div class="gridItemContainer">
      <div class="itemLink">
	<div class="itemBox">
	  <div class="itemImage">
	    {#if item === null}
	    {:else if item.typ === FileType.Folder}
	      <div class="folderlabel">{item.name}</div>
	      <a class="folder itemLink" href="{uiUrl({'view':'folder', 'dir':`${cleanDir}/${item.name}`, 'cfg':`${(cfg2str(queryCfg))}`})}">
		{#if item.media !== null}
		  <img class="gridImage folderimg" loading="lazy"
		    src="{apiUrl('thumb', {'path':`${cleanDir}/${item.name}/${item.media}`})}">
		{/if}
	      </a>
	    {:else if item.typ === FileType.Image}
	      {#if queryCfg.raw}
		<a class="itemLink2"
		  href="{apiUrl(`src/${encodeURIComponent(item.name)}`, {'dir':`${cleanDir}/`})}">
		  <img class="gridImage image" loading="lazy"
		    src="{apiUrl('thumb', {'path':`${cleanDir}/${item.name}`})}">
		</a>
	      {:else}
		<a class="itemLink2"
		  href="{uiUrl({'view':'media', 'dir':queryDir, 'name':item.name, 'cfg':cfg2str(queryCfg)})}">
		  <img class="gridImage image" loading="lazy"
		    src="{apiUrl('thumb', {'path':`${cleanDir}/${item.name}`})}">
		</a>
	      {/if}
	    {/if}
	  </div>
	</div>
      </div>
    </div>
  {/each}
{/if}
</div>
</div>

<!--
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
-->

</div>

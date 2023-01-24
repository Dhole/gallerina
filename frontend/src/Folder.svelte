<script>
  import { FileType, serverUrl, apiUrl, uiUrl, emptyCfg, cfg2str, str2cfg, trimPrefix, isImg } from './globals.ts';
  import { onMount, beforeUpdate } from 'svelte';
  import Nav from './Nav.svelte';

  let queryDirSplit = [];
  let queryDir = "";
  let queryPage = null;
  let totalPages = null;
  let footerPages = null;
  let cleanDir = "";
  let queryCfg = emptyCfg;
  let paramItems = [];

  function reload() {
    let base = window.location.origin + window.location.pathname;
    window.location.replace(`${base}?view=folder&dir=${queryDir}&page=${queryPage}&cfg=${cfg2str(queryCfg)}`);
  }

  function updateUrl() {
    let base = window.location.pathname;
    history.replaceState(null, '', `${base}?view=folder&dir=${queryDir}&page=${queryPage}&cfg=${cfg2str(queryCfg)}`);
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
    var page = parseInt(urlParams.get('page'), 10);
    page = isNaN(page) ? 0 : page;
    queryPage = page;
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
      let folderUrl = apiUrl('folder', {'sort':cfg.sort, 'reverse':cfg.reverse, 'dir':dir, 'page':page, 'seed':cfg.randSeed});
      if (dir !== "/") {
	folderUrl = folderUrl.replace(/\/$/, '');
      }
      const res = await fetch(folderUrl);
      let folder = await res.json();
      totalPages = Math.ceil(folder.total / folder.page_size);
      // if (cfg.sort === "random") {
      //   shuffleArray(folder.media, cfg.randSeed);
      // }
      folder.folders.forEach((folder) => {
	items.push({typ: FileType.Folder, name: folder.name, media: folder.media});
      })
      folder.media.forEach((media) => {
	items.push({typ: FileType.Image, name: media.name});
      });
    } else {
      let folderUrl = apiUrl('folderRecursive', {'sort':cfg.sort, 'dir':dir, 'page':page, 'seed':cfg.randSeed});
      if (dir !== "/") {
	folderUrl = folderUrl.replace(/\/$/, '');
      }
      const res = await fetch(folderUrl);
      let folder = await res.json();
      totalPages = Math.ceil(folder.total / folder.page_size);

      // if (cfg.sort === "random") {
      //   shuffleArray(folder.media, cfg.randSeed);
      // }
      folder.media.forEach((media) => {
	let name = `${trimPrefix(media.dir, dir)}/${media.name}`;
	name = trimPrefix(name, "/");
	items.push({typ: FileType.Image, name: name});
      });
    }
    totalPages = totalPages > 0 ? totalPages : 1;
    footerPages = [-2, -1, 0, 1, 2].map((i) => queryPage + i).filter((p) => p >= 0 && p < totalPages);
    paramItems = items;
  });
</script>

<div class="container">
  <Nav/>

  <h1>
{#each queryDirSplit as elem}
  <a href="{uiUrl({'view':'folder', 'dir':elem.dir, 'page':0, 'cfg':cfg2str(queryCfg)})}">
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
	  {#if item === null}
	  {:else if item.typ === FileType.Folder}
	  <a class="itemImage" href="{uiUrl({'view':'folder', 'dir':`${cleanDir}/${item.name}`, 'page':0, 'cfg':`${(cfg2str(queryCfg))}`})}">
	      <div class="folderlabel">{item.name}</div>
	      <div class="folder itemLink">
		{#if item.media !== null}
		<img class="gridImage folderimg" loading="lazy"
		  src="{apiUrl('thumb', {'path':`${cleanDir}/${item.name}/${item.media}`})}">
		{/if}
	      </div>
	  </a>
	  {:else if item.typ === FileType.Image}
	  <div class="itemImage">
	      {#if queryCfg.raw}
	      <a class="itemLink2"
		href="{apiUrl(`src/${encodeURIComponent(item.name)}`, {'dir':`${cleanDir}/`})}">
		{#if isImg(item.name)}
		<img class="gridImage image" loading="lazy"
		  src="{apiUrl('thumb', {'path':`${cleanDir}/${item.name}`})}">
		{:else}
		<img class="gridImage video" loading="lazy"
		  src="{apiUrl('thumb', {'path':`${cleanDir}/${item.name}`})}">
		{/if}
	      </a>
	      {:else}
	      <a class="itemLink2"
		href="{uiUrl({'view':'media', 'dir':queryDir, 'name':item.name, 'page':queryPage, 'cfg':cfg2str(queryCfg)})}">
		{#if isImg(item.name)}
		<img class="gridImage image" loading="lazy"
		  src="{apiUrl('thumb', {'path':`${cleanDir}/${item.name}`})}">
		{:else}
		<img class="gridImage video" loading="lazy"
		  src="{apiUrl('thumb', {'path':`${cleanDir}/${item.name}`})}">
		{/if}
	      </a>
	      {/if}
	  </div>
	  {/if}
	</div>
      </div>
    </div>
  {/each}
{/if}
</div>
<!-- footer -->
<hr style="margin: 3rem 0;">
<div class="caption">
  {#if footerPages}
    {#if queryPage === 0}
      <a class="button long disabled">⬅</a>
    {:else}
      <a href="{uiUrl({'view':'folder', 'dir':queryDir, 'page':queryPage-1, 'cfg':cfg2str(queryCfg)})}" class="button long primary">⬅</a>
    {/if}
    {#if footerPages[0] != 0}
      <a href="{uiUrl({'view':'folder', 'dir':queryDir, 'page':0, 'cfg':cfg2str(queryCfg)})}" class="button primary">0</a>
      <a class="button disabled clear">...</a>
    {/if}
    {#each footerPages as page}
      {#if page === queryPage}
	<a class="button disabled outline">{page}</a>
      {:else}
	<a href="{uiUrl({'view':'folder', 'dir':queryDir, 'page':page, 'cfg':cfg2str(queryCfg)})}" class="button primary">{page}</a>
      {/if}
    {/each}
    {#if footerPages[footerPages.length-1] != totalPages-1}
      <a class="button disabled clear">...</a>
      <a href="{uiUrl({'view':'folder', 'dir':queryDir, 'page':totalPages-1, 'cfg':cfg2str(queryCfg)})}" class="button primary">{totalPages-1}</a>
    {/if}
    {#if queryPage === totalPages-1}
      <a class="button long disabled">➡</a>
    {:else}
      <a href="{uiUrl({'view':'folder', 'dir':queryDir, 'page':queryPage+1, 'cfg':cfg2str(queryCfg)})}" class="button long primary">➡</a>
    {/if}
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

<script>
  import { FileType, serverUrl, apiUrl, uiUrl, defaultPlaySecs, emptyCfg, cfg2str, str2cfg, trimPrefix, isImg } from './globals.ts';
  import { onMount, beforeUpdate } from 'svelte';

  let queryPathSplit = [];
  // let paramItems = [];
  let queryDir = "";
  let queryPage = null;
  let total = null;
  let pageSize = null;
  let totalPages = null;
  let cleanDir = "";
  let queryName = "";
  let queryCfg = emptyCfg;
  let pages = null;
  // mediasData is an array of length mediasLength that either has null or a
  // promise with the image blob.
  let mediasData = [];
  // fetchController is an array of length imageLength that either has null or a
  // controller to cancel the fetch of the corresponding image.
  let fetchController = [];
  let fetchProgress = [];
  let mediasLength = -1;
  let index = -1;
  let indexP1 = -1;
  let fetchIndex = -1;
  let buttonPlay = false;
  let playSecs = defaultPlaySecs;
  let remSecs = 0;
  let playTimeout = undefined;
  let status = "[.]";

  const rowSize = 8;

  function fullscreen(elem) {
    if (elem.requestFullscreen) {
      elem.requestFullscreen();
    } else if (elem.webkitRequestFullscreen) { /* Safari */
      elem.webkitRequestFullscreen();
    } else if (elem.msRequestFullscreen) { /* IE11 */
      elem.msRequestFullscreen();
    }
  }

  function fullScreenElement() {
    return document.fullScreenElement || document.mozFullScreenElement || document.webkitIsFullScreenElement;
  }

  function fullscreenDoc() {
    let elem = document.documentElement;
    fullscreen(elem);
  }

  function fullscreenMedia() {
    let elem = document.getElementById("imgbar"); 
    fullscreen(elem);
  }

  function clickMedia(e) {
    let img = document.getElementById("media");
    if (img.src === "") {
      return;
    }
    let elem = fullScreenElement();
    if (elem === undefined) {
      fullscreenMedia();
    } else if (elem.id === "imgbar"){
      if (e.clientX <= window.innerWidth / 2) {
	loadPrev();
      } else {
	loadNext();
      }
    } else {
      fullscreenMedia();
    }
  }

  function imgUrl(name) {
    let u = apiUrl(`src/${encodeURIComponent(name)}`, {'dir':`${cleanDir}/`});
    return u;
  }

  function urlPath() {
    return uiUrl({'view':'media', 'cfg':cfg2str(queryCfg), 'dir':queryDir, 'name':queryName, 'page':queryPage});
  }

  const cachePrev = 2;
  const cacheNext = 5;

  function progressToChar(ind) {
    if (fetchProgress[ind] === 0) {
      return '.';
    } else if (fetchProgress[ind] === 1) {
      return 'o';
    } else {
      return '*';
    }
  }

  function updateStatus() {
    let cur = progressToChar(index)
    let next = [];
    for (var i = index + 1; i < Math.min(mediasLength, index + cacheNext + 1); i++) {
      next.push(progressToChar(i));
    }
    let prev = [];
    for (var i = index - 1; i >= Math.max(0, index - cachePrev);  i--) {
      prev.push(progressToChar(i));
    }
    prev.reverse();
    let _status = ""
    prev.forEach((c) => _status = _status + c);
    _status = _status + '[' + cur + ']';
    next.forEach((c) => _status = _status + c);
    status = _status;
  }

  let cancelFetch = async(ind) => {
    fetchController[ind].abort();
    fetchController[ind] = null;
    fetchProgress[ind] = 0;
    await mediasData[ind];
    mediasData[ind] = null;
  }

  let clearCache = async(ind) => {
    var toClear = [];
    // Clear before cachePrev
    for (var i = 0; i < ind - cachePrev; i++) {
      toClear.push(i);
    }
    // Clear after cacheNext
    for (var i = ind + cacheNext + 1; i < mediasLength; i++) {
      toClear.push(i);
    }
    for (var i = 0; i < toClear.length; i++) {
      const ind = toClear[i];
      if (mediasData[ind] !== null) {
	await cancelFetch(ind);
      }
    }
  }

  let fetchNext = async () => {
    var toFetch = [];
    for (var i = index + 1; i < Math.min(mediasLength, index + cacheNext + 1); i++) {
      toFetch.push(i);
    }
    for (var i = index - 1; i >= Math.max(0, index - cachePrev);  i--) {
      toFetch.push(i);
    }
    for (var i = 0; i < toFetch.length; i++) {
      const ind = toFetch[i];
      if (mediasData[ind] === null) {
	await fetchImgBlob(ind, false);
	return;
      }
    }
  }

  let readBodyProgress = async(res, ind) => {
    const length = parseInt(res.headers.get('Content-Length'), 10);
    if (!length) {
      const blob = await res.blob();
      fetchProgress[ind] = 1;
      return blob;
    }
    let at = 0
    let err = null;
    let _res = new Response(new ReadableStream({
      async start(controller) {
	const reader = res.body.getReader();
	for (;;) {
	  try {
	    const {done, value} = await reader.read();
	    if (done) break;
	    at += value.byteLength;
	    fetchProgress[ind] = at / length;
	    controller.enqueue(value);
	  } catch (error) {
	    err = error;
	    break;
	  }
	}
	controller.close();
      },
    }));
    const blob = await _res.blob();
    if (err !== null) {
      throw err
    }
    return blob;
  }

  // priorize sets the priority of fetching this image to the highest
  let fetchImgBlob = async (ind, priorize) => {
    if (mediasData[ind] !== null) {
      // If have priority and this image hasn't been fetched to 100%, cancel
      // the rest of the fetches in progress except this image, and then
      // schedule them again (with fetchNext).
      if (priorize && fetchProgress[ind] < 1) {
	for (var i = 0; i < mediasLength; i++) {
	  if (i !== ind && mediasData[i] !== null && fetchProgress[i] < 1) {
	    await cancelFetch(i);
	  }
	}
      }
      fetchNext();
      return;
    }
    if (priorize) {
      // If have priority and this image hasn't been fetched, just cancel all
      // the existing fetches in progress.
      for (var i = 0; i < mediasLength; i++) {
	if (mediasData[i] !== null && fetchProgress[i] < 1) {
	  await cancelFetch(i);
	}
      }
    }
    const media = await getMedia(ind)
    const name = media.name;
    const url = imgUrl(name);
    fetchIndex = ind;
    var controller = new AbortController();
    fetchController[ind] = controller;
    fetchProgress[ind] = 0;
    mediasData[ind] = new Promise(async(resolve, reject) => {
      try {
	var blob = undefined;
	if (isImg(name)) {
	  let res = await fetch(url, {signal: controller.signal});
	  // let blob = await res.blob();
	  blob = await readBodyProgress(res, ind);
	} else {
	  blob = await fetchVideoBlob();
	  fetchProgress[ind] = 1;
	}
	updateStatus();
	fetchNext();
	resolve(blob);
      } catch (err) {
	resolve(null);
      }
    });
  }

  let fetchVideoBlob = async() => {
    return "video";
  }

  let getImgBlob = async (ind) => {
    await clearCache(ind);
    await fetchImgBlob(ind, true);
    return mediasData[ind];
  }

  let getPage = async(page) => {
    let items = [];
    let _pageSize = 0;
    let _total = 0;
    let cfg = queryCfg;
    let dir = queryDir;
    if (cfg.recursive === false) {
      let folderUrl = apiUrl('folder', {'sort':cfg.sort, 'reverse':cfg.reverse, 'dir':dir, 'page':page, 'seed':cfg.randSeed});
      try {
	const res = await fetch(folderUrl);
	let _folder = await res.json();
	_total = _folder.total;
	_pageSize = _folder.page_size;
	items = _folder.media;
      } catch(err) {
	return;
      }
    } else {
      let folderUrl = apiUrl('folderRecursive', {'sort':cfg.sort, 'dir': dir, 'page':page, 'seed':cfg.randSeed});
      const res = await fetch(folderUrl);
      let _folder = await res.json();
      _total = _folder.total;
      _pageSize = _folder.page_size;
      _folder.media.forEach((media) => {
	let name = `${trimPrefix(media.dir, dir)}/${media.name}`;
	name = trimPrefix(name, "/");
	items.push({ name: name});
      });
    }
    return { items: items, pageSize: _pageSize, total: _total };
  }

  let getMedia = async (ind) => {
    let page = Math.floor(ind / pageSize);
    if (pages[page] === null) {
      pages[page] = (await getPage(page)).items;
    }
    return pages[page][ind % pageSize];
  }

  let loading = -1;
  let load = async (ind) => {
    loading += 1;
    const media = await getMedia(ind);
    queryName = media.name;
    window.history.replaceState({}, null, urlPath());
    var urlCreator = window.URL || window.webkitURL;
    let blob = await getImgBlob(ind);
    if (blob === null || ind !== index) {
      loading -= 1;
      return;
    }
    let img = document.getElementById("media");
    if (isImg(queryName)) {
      var imageUrl = urlCreator.createObjectURL(blob);
      img.src = imageUrl;
    } else {
      let source = document.getElementById("videosource");
      source.src = imgUrl(queryName);
      source.type = "video/mp4";
      img.load();
    }
    loading -= 1;
    updateStatus();
  }

  let checkKey = async(e) => {
    e = e || window.event;
    if (e.keyCode == '38') {
        // up arrow
    }
    else if (e.keyCode == '40') {
        // down arrow
    }
    else if (e.keyCode == '37') {
       // left arrow
      await loadPrev();
    }
    else if (e.keyCode == '39') {
       // right arrow
      await loadNext();
    }
  }
  document.onkeydown = checkKey;

  let loadIndexP1 = async (e) => {
    if (!e) e = window.event;
    var keyCode = e.code || e.key;
    if (keyCode !== 'Enter'){
      // Enter not pressed
      return false;
    }

    indexP1 = parseInt(indexP1, 10);
    if (isNaN(indexP1)) {
      indexP1 = index + 1;
      return;
    }
    let newIndex = indexP1 - 1;
    if (newIndex < 0 || newIndex > mediasLength - 1) {
      indexP1 = index + 1;
      return;
    }
    index = newIndex;
    await load(index);
  };

  let loadNext = async () => {
    if (index < mediasLength - 1) {
      index += 1;
      indexP1 = index+1;
      if (index === mediasLength - 1) {
	buttonPlay = false;
      }
    } else {
      buttonPlay = false;
      return;
    }
    await load(index);
  };

  let loadPrev = async () => {
    if (index > 0) {
      index -= 1;
      indexP1 = index+1;
    }
    await load(index);
  };

  function updateProgressRing() {
    var circle = document.getElementById('progress-ring');
    var radius = circle.r.baseVal.value;
    var circumference = radius * 2 * Math.PI;
    const offset = circumference - (1 - (remSecs / playSecs)) * circumference;
    circle.style.strokeDashoffset = offset;
  }

  let slideshowLoop = async () => {
    remSecs -= 1;
    updateProgressRing();
    if (remSecs <= 0) {
      await loadNext();
      remSecs = playSecs;
      updateProgressRing();
    }
    if (buttonPlay) {
      playTimeout = setTimeout(slideshowLoop, 1000);
    }
  }

  let play = async () => {
    playSecs = parseInt(playSecs, 10);
    if (isNaN(playSecs) || playSecs < 1) {
      playSecs = defaultPlaySecs;
    }
    if (index === mediasLength - 1) {
      return;
    }
    buttonPlay = true;
    remSecs = playSecs;

    let circle = document.getElementById('progress-ring');
    let radius = circle.r.baseVal.value;
    let circumference = radius * 2 * Math.PI;

    circle.style.strokeDasharray = `${circumference} ${circumference}`;
    updateProgressRing();
    document.getElementById('div-progress-ring').style.display = "block";

    playTimeout = setTimeout(slideshowLoop, 1000);
  };

  let pause = async () => {
    buttonPlay = false;
    clearTimeout(playTimeout);
    document.getElementById('div-progress-ring').style.display = "none";
  };

  onMount(async () => {
    // let param = window.location.hash.substr(1);
    let urlParams = new URLSearchParams(window.location.search);
    let dir = urlParams.get('dir');
    queryDir = dir;
    var page = parseInt(urlParams.get('page'), 10);
    queryPage = page;
    cleanDir = queryDir === "/" ? "" : queryDir;
    let name = urlParams.get('name');
    queryName = name;
    let cfg = urlParams.get('cfg');
    cfg = cfg == null ? emptyCfg : str2cfg(cfg);
    queryCfg = cfg;
    let pathSplit = decodeURIComponent(dir).split("/");
    for (let i = 0; i < pathSplit.length - 1; i++) {
      pathSplit[i] = `${pathSplit[i]}/`;
    }
    queryPathSplit = pathSplit;

    let _page = await getPage(page);
    let items = _page.items;
    total = _page.total;
    pageSize = _page.pageSize;
    totalPages = Math.ceil(total / pageSize);
    // if (cfg.sort === "random") {
    //   shuffleArray(items, cfg.randSeed);
    // }
    pages = new Array(totalPages).fill(null);
    pages[queryPage] = items;
    mediasLength = total;
    mediasData = new Array(mediasLength).fill(null);
    fetchController = new Array(mediasLength).fill(null);
    fetchProgress = new Array(mediasLength).fill(0);
    loading = 0;
    index = items.findIndex((media) => { return media.name === queryName; }) + queryPage * pageSize;
    indexP1 = index+1;
    await load(index);
  });
</script>

<!--<div style="position:device-fixed;height: 100vh;">-->
<div style="position: static;">

  <div id="div-progress-ring" style="position:fixed; top: 1em; right: 1em; display: none; z-index: 2;">
  <!-- Circular bar -->
  <svg
     class="progress-ring"
     width="100"
     height="100">
    <circle
      fill="#282828"
      r="50"
      cx="50"
      cy="50"/>
    <circle
      id="progress-ring"
      class="progress-ring-circle"
      stroke="#B0DEFF"
      stroke-width="16"
      fill="transparent"
      r="42"
      cx="50"
      cy="50"/>
    <text  x="50%" y="50%" dominant-baseline="middle" text-anchor="middle" fill="#d2d6dd">{remSecs}</text>
  </svg>
</div>

  <div style="align-items: center;">
    <div class="imgcontainer">
      <div class="imgbar" id="imgbar" on:click={clickMedia}>
	{#if queryName !== ""}
	  {#if isImg(queryName)}
	    <img id="media" class="imgview" alt="{queryName}">
	  {:else}
	    <video id="media" class="imgview" style="width: 90%" alt="{queryName}" controls>
	      <source id="videosource">
	    </video>
	  {/if}
	{:else}
	  loading...
	{/if}
      </div>
    </div>
      <div class="caption">
	{#if loading > 0}
	  <p style="margin: 0;">loading {queryName} {Math.round(fetchProgress[index] * 100)}%...</p>
	{:else}
	  <p style="margin: 0;"><a class="imgname" href="{imgUrl(queryName)}">{queryName}</a></p>
	{/if}
      </div>
      <div class="caption">
	<div style="display: flex; margin: 0 auto;">
	  <a style="margin: 0 1em;" class="button primary" on:click={loadPrev}>&larr;</a>
	  {#if index !== -1}
	    <input style="width: {3+Math.log10(mediasLength)}ch; text-align: right;"type="text" id="imgnum" name="imgnum" on:keypress={loadIndexP1} bind:value={indexP1}>
	  {:else}
	    <input style="width: 2em; text-align: right;"type="text" id="imgnum" name="imgnum" value="?">
	  {/if}
	  {#if mediasLength !== -1}
	    <div class="label">/{mediasLength}</div>
	  {:else}
	    <div class="label">/?</div>
	  {/if}
	  <a style="margin: 0 1em;" class="button primary" on:click={loadNext}>&rarr;</a>
	</div>
	{#if window.innerWidth >= 600}
	  <div style="float: right; position: absolute; right: 1em">
	    <div style="display: flex">

	      {#if buttonPlay}
		<a class="button primary" on:click={pause}>⏸</a>
	      {:else}
		<a class="button primary" on:click={play}>▶️</a>
	      {/if}
	      <input style="margin-left: 1em; width: 3em; text-align: right;" type="text" id="playsecs" name="playsecs" bind:value={playSecs}>
	      <div class="label">secs</div>
	      <div style="margin-left: 1em; display: flex">
		  <a class="button primary" on:click={fullscreenDoc}>↕</a>
	      </div>
	    </div>
	  </div>
	  <div style="float: left; position: absolute; left: 1em">
	      <code>
		{status}
	      </code>
	  </div>
	{:else}
	  <div style="float: right; position: absolute; right: 1em">
	    <div style="display: flex">
		<a class="button primary" on:click={fullscreenDoc}>↕</a>
	    </div>
	  </div>
	{/if}
      </div>
  </div>

</div>

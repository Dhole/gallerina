<script>
  import { FileType, serverUrl, defaultPlaySecs } from './globals.ts';
  import { onMount, beforeUpdate } from 'svelte';

  let queryPathSplit = [];
  let paramFileRows = [];
  let queryDir = "";
  let queryName = "";
  let querySort = "";
  let queryReverse = false;
  let folder = [];
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
    let elem = document.getElementById("media"); 
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
    } else if (elem.id === "media"){
      if (e.clientX <= window.innerWidth / 2) {
	loadPrev();
      } else {
	loadNext();
      }
    } else {
      fullscreenMedia();
    }
  }

  function reload() {
    let base = window.location.origin + window.location.pathname;
    window.location.replace(`${base}?view=folder&sort=${querySort}&reverse=${queryReverse}&dir=${queryDir}&name=${queryName}`);
  }

  function imgUrl(name) {
    return `${serverUrl}/src/${name}?dir=${queryDir}`;
  }

  function urlPath() {
    return `?view=media&sort=${querySort}&reverse=${queryReverse}&dir=${queryDir}&name=${queryName}`;
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
    // console.log("cancelFetch", "index", ind);
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
    // console.log("index", ind, "toClear", toClear);
    for (var i = 0; i < toClear.length; i++) {
      const ind = toClear[i];
      if (mediasData[ind] !== null) {
	await cancelFetch(ind);
      }
    }
  }

  let fetchNext = async () => {
    // console.log("fetchNext");
    var toFetch = [];
    for (var i = index + 1; i < Math.min(mediasLength, index + cacheNext + 1); i++) {
      toFetch.push(i);
    }
    for (var i = index - 1; i >= Math.max(0, index - cachePrev);  i--) {
      toFetch.push(i);
    }
    // console.log("toFetch", toFetch);
    for (var i = 0; i < toFetch.length; i++) {
      const ind = toFetch[i];
      if (mediasData[ind] === null) {
	// console.log("schedule", "index", ind);
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
      // console.log("mediasData not null");
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
    const name = folder.media[ind].name;
    const url = imgUrl(name);
    fetchIndex = ind;
    var controller = new AbortController();
    fetchController[ind] = controller;
    // mediasData[ind] = fetch(url, {signal: controller.signal}).then(res => {
    //   return res.blob();
    //   // return readBodyProgress(res, ind);
    // }).then(blob => {
    //   console.log("fetched", "index", ind, "url", url);
    //   fetchNext();
    //   return blob;
    // }).catch(error => {
    //   // console.log("fetch errror", error);
    //   return null;
    // })
    fetchProgress[ind] = 0;
    mediasData[ind] = new Promise(async(resolve, reject) => {
      try {
	let res = await fetch(url, {signal: controller.signal});
	// let blob = await res.blob();
	let blob = await readBodyProgress(res, ind);
	updateStatus();
	// console.log("fetched", "index", ind, "url", url);
	fetchNext();
	resolve(blob);
      } catch (err) {
	console.log("fetch imgUrl", err);
	resolve(null);
      }
    });
    // console.log("mediasData assigned");
  }

  let getImgBlob = async (ind) => {
    await clearCache(ind);
    await fetchImgBlob(ind, true);
    return mediasData[ind];
  }

  let loading = -1;
  let load = async (ind) => {
    loading += 1;
    queryName = folder.media[ind].name;
    window.history.replaceState({}, null, urlPath());
    var urlCreator = window.URL || window.webkitURL;
    let blob = await getImgBlob(ind);
    // console.log("blob", blob);
    if (blob === null || ind !== index) {
      loading -= 1;
      return;
    }
    var imageUrl = urlCreator.createObjectURL(blob);
    let img = document.getElementById("media");
    img.src = imageUrl;
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

  let slideshowLoop = async () => {
    await loadNext();
    playTimeout = setTimeout(slideshowLoop, playSecs * 1000);
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
    playTimeout = setTimeout(slideshowLoop, playSecs * 1000);
  };

  let pause = async () => {
    buttonPlay = false;
    clearTimeout(playTimeout);
  };

  onMount(async () => {
    // let param = window.location.hash.substr(1);
    let urlParams = new URLSearchParams(window.location.search);
    let dir = urlParams.get('dir');
    queryDir = dir;
    let name = urlParams.get('name');
    queryName = name;
    // console.log("name", queryName);
    let sort = urlParams.get('sort');
    querySort = sort;
    let reverse = urlParams.get('reverse');
    queryReverse = reverse === "true" ? true : false;
    let pathSplit = decodeURIComponent(dir).split("/");
    for (let i = 0; i < pathSplit.length - 1; i++) {
      pathSplit[i] = `${pathSplit[i]}/`;
    }
    // console.log(pathSplit);
    queryPathSplit = pathSplit;

    let folderUrl = `${serverUrl}/folder?sort=${querySort}&reverse=${queryReverse}&dir=${dir}`;
    if (dir !== "/") {
      folderUrl = folderUrl.replace(/\/$/, '');
    }
    try {
      const res = await fetch(folderUrl);
      folder = await res.json();
    } catch(err) {
      console.log("fetch folderUrl", err);
      return;
    }
    // console.log(folder.media);
    mediasLength = folder.media.length;
    mediasData = new Array(mediasLength).fill(null);
    fetchController = new Array(mediasLength).fill(null);
    fetchProgress = new Array(mediasLength).fill(0);
    loading = 0;
    index = folder.media.findIndex((media) => { return media.name === queryName; });
    indexP1 = index+1;
    // console.log(folder);
    let fileRows = [[]];
    let row = 0;
    folder.folders.forEach((folder) => {
      fileRows[row].push({typ: FileType.Folder, name: folder});
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
    await load(index);
    // console.log(fileRows);
  });
</script>

<!--<div style="position:device-fixed;height: 100vh;">-->
<div style="position: static;">
  <div style="align-items: center;">
    <div class="imgcontainer">
      <div class="imgbar">
	{#if queryName !== ""}
	  <!--<img id="media" src="{serverUrl}/src/{queryName}?dir={queryDir}" class="imgview" alt="{queryName}" on:click={clickMedia}>-->
	  <img id="media" class="imgview" alt="{queryName}" on:click={clickMedia}>
	{:else}
	  loading...
	{/if}
      </div>
    </div>
      <div class="caption">
	{#if loading > 0}
	  <p style="margin: 0;">loading {queryName} {Math.round(fetchProgress[index] * 100)}%...</p>
	{:else}
	  <p style="margin: 0;">{queryName}</p>
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

  <!--
  <div class="row">
    <div class="col">
      <img style="display: flex; margin: 0 auto; height: 100%; object-fit: contain;" src="{serverUrl}/src?dir={queryPath}">
    </div>
  </div>

  <div style="border: 0.1em solid; border-radius: 0.2em; margin: 1em 0.1em;" class="row">
    <div style="display: flex; margin: 0.4em 1em;" class="col">
      <a class="button primary">Previous</a>
      <label> 1/24 </label>
      <a class="button primary">Next</a>
    </div>
  </div>
  -->


</div>

<script>
  import { FileType, serverUrl } from './globals.ts';
  import { onMount, beforeUpdate } from 'svelte';
  import Nav from './Nav.svelte';

  let status = null;

  let scanRun = async() => {
    await fetch(`${serverUrl}/scanner/run`, {
      method: "POST",
      body: null
    }).then(res => {
      console.log("Request response:", res);
    })
      .catch(err => {
      console.log("Request error:", err);
    });
    await getStatus();
  }

  let scanStop = async() => {
    await fetch(`${serverUrl}/scanner/stop`, {
      method: "POST",
      body: null
    }).catch(err => {
      console.log("Request error:", err);
    });
    await getStatus();
  }

  let getStatus = async() => {
    const res = await fetch(`${serverUrl}/status`);
    let _status = await res.json();
    if (typeof _status.scanner_state.Error === "string") {
      _status.scanner_state = 'Error: ' + _status.scanner_state.Error;
    }
    status = _status;
    if (status.scanner_state.startsWith("Idle") || status.scanner_state.startsWith("Error")) {
      setUpdateStatusLoopInterval(10000);
    } else {
      setUpdateStatusLoopInterval(1000);
    }
    console.log("Status:", status);
  }

  function parseDate(s) {
    if (s === null) {
      return s;
    }
    let dateRe = RegExp(/([^T]+)T([^.]+)\.[0-9]*(.+)/mg);
    let match = dateRe.exec(s);
    return match[1] + ' ' + match[2] + match[3];
  }

  let updateStatusLoopTimer = null
  let updateStatusLoopInterval = 1000;
  function setUpdateStatusLoopInterval(ms) {
    if (updateStatusLoopTimer !== null) {
      clearInterval(updateStatusLoopTimer);
    }
    updateStatusLoopInterval = ms;
    updateStatusLoopTimer = setInterval(getStatus, updateStatusLoopInterval)
  }

  onMount(async () => {
    await getStatus();
    setUpdateStatusLoopInterval(1000)
  })

</script>

<div class="container">
  <Nav/>
  <br>

  <p>
  {#if status === null}
    loading...
  {:else}
    <ul>
      <li><b>Root:</b> {status.root}</li>
      <li><b>Scanner State:</b> {status.scanner_state}</li>
      <li><b>Stats:</b>
	<ul>
	  <li><b>Last Scan Start:</b> {parseDate(status.stats.last_scan_start)}</li>
	  <li><b>Last Scan End:</b> {parseDate(status.stats.last_scan_end)}</li>
	  <li><b>Indexed Folders:</b> {status.stats.scan_folders_count} / {status.stats.scan_folders_total} = 
	  {(status.stats.scan_folders_count / status.stats.scan_folders_total * 100).toFixed(2)}%</li>
	  <li><b>Indexed Files:</b> {status.stats.scan_files_count} / {status.stats.scan_files_total} = 
	  {(status.stats.scan_files_count / status.stats.scan_files_total * 100).toFixed(2)}%</li>
	</ul>
      </li>
    </ul>
  {/if}
  </p>

  <p>
  {#if status === null}
    loading...
  {:else}
    {#if status.scanner_state.startsWith("Idle") || status.scanner_state.startsWith("Error")}
      <a class="button primary success" on:click={scanRun}>Start Scan</a>
    {:else}
      <a class="button error" on:click={scanStop}>Stop Scan</a>
    {/if}
  {/if}
  </p>
</div>

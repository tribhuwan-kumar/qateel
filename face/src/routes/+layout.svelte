<script lang="ts">
	import "./layout.css";
	import { onMount } from "svelte";
	import { dev } from '$app/environment';
  import { ModeWatcher } from "mode-watcher";
	import { getSysStatus } from '$lib/system';
	import { checkAuth } from "$lib/auth/auth.svelte";
	import { startAuthWorker, stopAuthWorker  } from "$lib/auth/auth.svelte";
  import { Toaster } from "$lib/components/ui/sonner/index.js";
  import { historySort, loadHistorySort } from "$lib/sort";

	let { children } = $props();

	function handleVisibilityChange() {
		if (document.visibilityState === 'visible') {
			checkAuth();
		}
	}

	$effect(() => {
			startAuthWorker();
			// Cleanup automatically stop the worker
			return () => {
				stopAuthWorker();
			};
	});

	onMount(() => {
    document.getElementById("initial-loader")?.remove();
    document.getElementById("init-animation-style")?.remove();
    document.getElementById("init-loader-script")?.remove();
    getSysStatus();
    historySort.set(loadHistorySort());

		if (dev) {
			import('eruda').then((eruda) => {
				eruda.default.init();
			});
		}
  });
</script>

<svelte:head>
  <title>Qateel | Aria2 Web UI</title>
</svelte:head>

<svelte:window onvisibilitychange={handleVisibilityChange} />

<div class="app">
  <ModeWatcher />
	<main>
    {@render children()}
    <Toaster />
  </main>
</div>


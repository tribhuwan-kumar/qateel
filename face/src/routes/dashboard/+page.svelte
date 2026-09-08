<script lang="ts">
	import { onMount } from "svelte";
	import { fly } from 'svelte/transition';
	import { aria2, aria2Info } from '$lib/aria2/client.svelte'
	import Navbar from '$lib/components/Navbar.svelte';
	import Sidebar from "$lib/components/Sidebar.svelte";
	import DlTable from '$lib/components/DlTable.svelte';
	import * as SidebarUI from "$lib/components/ui/sidebar/index.js";

	async function getAria2Info(): Promise<void> {
		const info = await aria2.getAria2Info();
		aria2Info.set(info);
		console.log("Aria2 info:", $aria2Info);
	};

	onMount(async () => {
    await getAria2Info();
  });
</script>

<div class="flex flex-row w-full h-full min-h-svh"
	in:fly={{ y: 100, duration: 800 }}
>
	<SidebarUI.Provider class="w-full h-full min-h-svh">
		<Sidebar />
		<div class="flex min-h-svh w-full flex-col">
			<Navbar />
			<DlTable />
		</div>
	</SidebarUI.Provider>
</div>


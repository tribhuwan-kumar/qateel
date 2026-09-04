<script lang="ts">
import { fly } from 'svelte/transition';
import Navbar from '$lib/components/Navbar.svelte';
import Sidebar from "$lib/components/Sidebar.svelte";
import DlTable from '$lib/components/DlTable.svelte';
import * as SidebarUI from "$lib/components/ui/sidebar/index.js";
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


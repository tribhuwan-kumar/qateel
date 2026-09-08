<script lang="ts">
  import { formatSpeed } from "$lib/utils";
  import { systemState } from '$lib/system';
  import { Badge } from "$lib/components/ui/badge";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Button } from "$lib/components/ui/button";
	import AddDl from '$lib/components/AddDl.svelte';
  import * as Dialog from "$lib/components/ui/dialog";
	import { JsonView } from '@zerodevx/svelte-json-view'
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
	import * as SidebarUI from "$lib/components/ui/sidebar/index.js";
  import SearchDialog from "$lib/components/SearchDialog.svelte";
  import {
    aria2,
		aria2Store,
  } from '$lib/aria2/client.svelte';
  import {
    selectedGids,
    clearSelection,
  } from '$lib/selection';
	import {
		CodeIcon,
    PlayIcon,
    ListXIcon,
    PauseIcon,
		Menu04Icon,
    Search02Icon,
    Cancel02Icon,
    Delete02Icon,
		PlusSignIcon,
		Sorting05Icon,
    ArrowUpDoubleIcon,
    ArrowDownDoubleIcon,
    CheckmarkBadge03Icon,
  } from '@hugeicons/core-free-icons';

  let count = $derived($selectedGids.length);
  let globalStats = $derived(aria2Store.global);
  let hasSelection = $derived(count > 0);
  let isAria2Alive = $derived($systemState.status?.aria2Alive);
  let selectedItems = $derived(aria2Store.items.filter(i => $selectedGids.includes(i.gid)));

  let canResume = $derived(selectedItems.some(i => ['paused'].includes(i.status)));
  let canPause = $derived(selectedItems.some(i => ['active', 'waiting'].includes(i.status)));
  let canStop = $derived(selectedItems.some(i => ['active', 'waiting', 'paused', 'error'].includes(i.status)));

  let deleteDialogOpen = $state(false);
	let searchDialogOpen = $state(false);
  let deleteWithFiles = $state(false);
  let addDialogOpen = $state(false);
  let gidsToDelete = $state<string[]>([]);
  let showGlobalStatsDialog = $state(false);
  let globalStatsView: "table" | "json" = $state("table");

  function handleAdd() {
    addDialogOpen = true;
  }

  // Only `paused` and `stopped` gets resumed
  async function handleResume() {
    const targets = selectedItems.filter(i => ['paused', 'stopped'].includes(i.status));
		const gids = targets.map(i => (i.gid));
    await aria2.resume(gids);
    clearSelection();
  }

	// Only `active` and `waiting` can be paused,
	// `waiting` doesn't emits the notification event on being paused, had to it handle manually
  async function handlePause() {
    const targets = selectedItems.filter(i => ['active', 'waiting'].includes(i.status));
		const gids = targets.map(i => (i.gid));
    await aria2.pause(gids);
    clearSelection();
  }

  // Stopping -> removing from queue, freeing aria2 memory
  async function handleStop() {
    const targets = selectedItems.filter(i => ['active', 'waiting', 'paused', 'error'].includes(i.status));
		const gids = targets.map(i => (i.gid));
		await aria2.stop(gids);
    clearSelection();
  }

  /// Remove -> Delete
  function promptDelete() {
    if ($selectedGids.length === 0) return;
    gidsToDelete = $selectedGids;
    deleteWithFiles = false;
    deleteDialogOpen = true;
  }

  function confirmDelete() {
    aria2.delete(gidsToDelete, deleteWithFiles);
    deleteDialogOpen = false;
    clearSelection();
  }

  function handleSearch() {
		searchDialogOpen = true;
  }

  function handleSort() {

  }

  const globalStatLabels: Record<string, string> = {
      downloadSpeed: "Download speed",
      uploadSpeed: "Upload speed",
      numActive: "Active",
      numStopped: "Stopped",
      numStoppedTotal: "Stopped total",
      numWaiting: "Waiting"
  };

  function prettyKey(key: string) {
      return (
          globalStatLabels[key] ??
          key.replace(/([a-z0-9])([A-Z])/g, "$1 $2").replaceAll("_", " ")
      );
  }

  function formatGlobalStatValue(key: string, value: unknown) {
      if (key.toLowerCase().includes("speed")) {
          if (value === null || value === undefined || value === "" || value === "0") {
              return "0 KiB/s";
          }
          return formatSpeed(value as string);
      }
      if (value === null || value === undefined || value === "") return "—";
      return String(value);
  }

  function openGlobalStatsDialog() {
      globalStatsView = "table";
      showGlobalStatsDialog = true;
  }


  $effect(() => {
      console.log("Aria2 alive:", isAria2Alive);
  });
</script>


<div class="bg-background/95 backdrop-blur z-1 supports-backdrop-filter:bg-background/90 dark:supports-backdrop-filter:bg-background/60 sticky top-0 w-full">
	<div class="inline-flex flex-row">
		<SidebarUI.Trigger
			class="m-2 text-muted-foreground bg-background/95 backdrop-blur
				dark:supports-backdrop-filter:bg-background/60 supports-backdrop-filter:bg-background/90 sticky top-0 cursor-pointer"
		/>
		<div class="hidden sm:inline-flex justify-start float-left md:h-12 items-center md:px-4 gap-2">
			<div class="flex items-center gap-4 text-sm text-muted-foreground">
				{#if globalStats.downloadSpeed !== "0" || globalStats.uploadSpeed !== "0"}
					<Badge
						variant="outline"
						onclick={openGlobalStatsDialog}
						title="Global downloading speed"
						class="gap-1 px-2 py-3.5 min-w-26 cursor-pointer text-xs animate-in fade-in zoom-in duration-200"
					>
						{#if globalStats.downloadSpeed !== "0"}
							<HugeiconsIcon
								icon={ArrowDownDoubleIcon}
								size={24}
								color="currentColor"
								strokeWidth={2}
								class="w-8! h-8!"
							/>
							{formatSpeed(globalStats.downloadSpeed)}
						{/if}
						{#if globalStats.uploadSpeed !== "0"}
							<div class="w-px h-5 mx-2 bg-border"></div>
								<HugeiconsIcon
									icon={ArrowUpDoubleIcon}
									size={24}
									color="currentColor"
									strokeWidth={1.5}
								/>
							{formatSpeed(globalStats.uploadSpeed)}
						{/if}
					</Badge>
				{/if}
			</div>
		</div>
	</div>

  <div class="inline-flex justify-end float-right h-12 items-center px-4 gap-2">
    <div class="hidden sm:flex items-center text-sm text-muted-foreground">
      {#if hasSelection}
        <div class="flex items-center text-primary/90 gap-1 md:gap-2 border border-muted-foreground/20 px-2 py-1 rounded-full animate-in fade-in zoom-in duration-200">
          <HugeiconsIcon
            icon={CheckmarkBadge03Icon}
            size={18}
            color="currentColor"
            strokeWidth={1.5}
						class="md:w-4.5 w-4"
          />
          <span class="max-xs:text-[12px] text-xs">{count}&nbsp;&nbsp;Selected</span>
          <button
            onclick={clearSelection}
            class="md:ml-1 hover:bg-muted-foreground/20 rounded-xl p-0.5 cursor-pointer"
            title="Clear selection"
          >
            <HugeiconsIcon
              icon={Cancel02Icon}
              color="currentColor"
              strokeWidth={1.5}
							class="md:w-4.5 w-4"
            />
          </button>
        </div>
        <div class="w-px h-6 mx-2 bg-border"></div>
      {/if}
    </div>
    <Button
      onclick={handleSearch}
      disabled={!isAria2Alive}
      class="cursor-pointer! disabled:cursor-not-allowed!"
      size="icon"
      variant="ghost"
      title="Search"
    >
      <HugeiconsIcon
        icon={Search02Icon}
        class="text-muted-foreground w-4! h-4!"
        strokeWidth={2}
      />
    </Button>
    <Button
      onclick={handleSort}
      disabled={!isAria2Alive}
      class="cursor-pointer! disabled:cursor-not-allowed!"
      size="icon"
      variant="ghost"
      title="Sort"
    >
      <HugeiconsIcon
        icon={Sorting05Icon}
        class="text-muted-foreground w-4! h-4!"
        strokeWidth={2}
      />
    </Button>
		<!-- vertical seperator -->
    <div class="w-px h-6 mx-2 bg-border"></div>
    <Button
			disabled={!isAria2Alive}
      onclick={handleAdd}
      class="cursor-pointer disabled:cursor-not-allowed!"
      size="icon"
      variant="ghost"
      title="Add downloads"
    >
      <HugeiconsIcon
        icon={PlusSignIcon}
        class="text-muted-foreground w-4! h-4!"
        strokeWidth={2}
      />
    </Button>
    <AddDl
      bind:open={addDialogOpen}
      onclose={() => addDialogOpen = false}
    />
		<SearchDialog bind:open={searchDialogOpen} />
		<div class="w-px h-6 mx-2 bg-border hidden md:block"></div>
		<!-- md screen action buttons -->
    <div class="items-center gap-1 hidden md:flex">
      <Button
        variant="ghost"
        size="icon"
				class="disabled:cursor-not-allowed! cursor-pointer"
        disabled={!hasSelection || !isAria2Alive || !canResume}
        onclick={handleResume}
        title="Resume selected"
      >
        <HugeiconsIcon
          icon={PlayIcon}
          class="text-muted-foreground w-4! h-4!"
          strokeWidth={2}
          size={24}
        />
      </Button>

      <Button
        variant="ghost"
        size="icon"
        disabled={!hasSelection || !isAria2Alive || !canPause}
        class="disabled:cursor-not-allowed! cursor-pointer"
        onclick={handlePause}
        title="Pause selected"
      >
        <HugeiconsIcon
          icon={PauseIcon}
          class="text-muted-foreground w-4! h-4!"
          strokeWidth={2}
          size={24}
        />
      </Button>

      <Button
        variant="ghost"
        size="icon"
        disabled={!hasSelection || !isAria2Alive || !canStop}
        class="text-muted-foreground cursor-pointer! disabled:cursor-not-allowed!"
        onclick={handleStop}
        title="Stop from downloading"
      >
        <HugeiconsIcon
          icon={ListXIcon}
          class="w-4! h-4!"
          strokeWidth={2}
          size={24}
        />
      </Button>

      <Button
        size="icon"
        variant="ghost"
        class="text-muted-foreground hover:text-destructive! cursor-pointer! disabled:cursor-not-allowed! hover:bg-destructive/10!"
        disabled={!hasSelection || !isAria2Alive}
        onclick={promptDelete}
        title="Delete selected"
      >
        <HugeiconsIcon
          size={24}
          strokeWidth={2}
          icon={Delete02Icon}
          class="w-4! h-4!"
        />
      </Button>

			<div class="w-px h-6 mx-2 bg-border"></div>
			<Button
				class="cursor-pointer! disabled:cursor-not-allowed!"
				size="icon"
				variant="ghost"
				title="More"
			>
			<!-- need: -->
			<!-- pauseAll -->
			<!-- resumeAll -->
			<!-- StopAll -->
			<!-- purgeDownloadResult -> removes completed/error/removed -->
				<HugeiconsIcon
					icon={Menu04Icon}
					class="text-muted-foreground w-4! h-4!"
					strokeWidth={2}
				/>
			</Button>
    </div>
  </div>
</div>

<Dialog.Root bind:open={deleteDialogOpen}>
  <Dialog.Content showCloseButton={false} class="sm:max-w-[500px] sm:min-h-[300px]">
    <Dialog.Header>
      <Dialog.Title>Confirm Delete</Dialog.Title>
      <Dialog.Description>
        Are you sure you want to delete this download?
        <br/> this action cannot be undone.
        <br/><br/>
      </Dialog.Description>
    </Dialog.Header>

    <div class="flex flex-row gap-2">
      <div class="w-4 h-6 mt-1 inline">
        <Checkbox
          class="cursor-pointer"
          id="delete-with-files"
          bind:checked={deleteWithFiles}
        />
      </div>
      <div class="inline">
        <Label
          for="delete-with-files"
          class="cursor-pointer"
        >
          Delete files too!?
        </Label>
				<br/>
        <span class="text-[14px] font-normal text-muted-foreground">be careful, it'll delete their actual content from server!!</span>
      </div>
    </div>

    <Dialog.Footer
      class="mt-2"
    >
      <Button
        class="cursor-pointer"
        variant="outline"
        onclick={() => deleteDialogOpen = false}
      >
        Cancel
      </Button>
      <Button
        class="min-w-24 cursor-pointer"
        variant="destructive"
        onclick={confirmDelete}
      >
        Delete
      </Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>

<Dialog.Root
	open={showGlobalStatsDialog}
	onOpenChange={(v) => showGlobalStatsDialog = v}
>
	<Dialog.Content
		showCloseButton={false}
		class="w-4xl h-78 max-xs:h-85 md:max-h-76 bg-background/95 dark:bg-background/60
			supports-backdrop-filter:bg-background/90	dark:supports-backdrop-filter:bg-background/60 backdrop-blur-xl"
	>
		<Dialog.Header class="flex pb-1 flex-row items-start justify-between relative after:absolute after:bottom-0
			after:left-0 after:h-[1px] after:w-full after:bg-gradient-to-r after:from-transparent after:via-sidebar-border after:to-transparent
		">
			<div>
					<Dialog.Title>Global stats</Dialog.Title>
					<Dialog.Description class="mt-1">
							All current stats in real time!!
					</Dialog.Description>
			</div>
			<button
				title="JSON view"
				onclick={() => globalStatsView = globalStatsView === "table" ? "json" : "table"}
				class={`cursor-pointer! w-8 h-8 flex justify-center items-center rounded-full ${globalStatsView === "json" ? "bg-muted" : ""}`}
			>
				<HugeiconsIcon
					icon={CodeIcon}
					class="w-4 h-4"
					strokeWidth={2}
				/>
			</button>
		</Dialog.Header>
			{#if globalStatsView === "table"}
				<div class="min-h-fit rounded-xl border">
					<table class="w-full text-[16px]">
						<tbody>
							{#each Object.entries(globalStats ?? {}) as [key, value] (key)}
								<tr class="border-b last:border-b-0">
									<th class="w-1/2 px-4 bg-muted/40 text-xs py-2 font-normal text-left text-muted-foreground">
											{prettyKey(key)}
									</th>
									<td class="px-4 py-1 text-xs break-all">
											{formatGlobalStatValue(key, value)}
									</td>
								</tr>
							{/each}
						</tbody>
					</table>
				</div>
			{:else}
				<div class="min-h-[200px] overflow-x-scroll overflow-y-scroll z-100 json-class overflow-auto rounded-xl border bg-muted/40 p-4 text-xs">
					<JsonView
						json={globalStats}
					/>
				</div>
			{/if}
	</Dialog.Content>
</Dialog.Root>


<style>
  .json-class {
    font-family: monospace;
    font-size: 14px;
    --jsonBorderLeft: 1px solid var(--color-muted-foreground);
    --jsonValColor: var(--color-muted-foreground);
		--jsonBracketColor: var(--bg-muted);
		--jsonBracketHoverBackground: var(--color-muted);
		--jsonValColor: var(--color-muted-foreground);
		--jsonValBooleanColor: var(--color-muted-foreground);
		--jsonValStringColor: var(--color-muted-foreground);
		--jsonValNumberColor: var(--color-muted-foreground);
  }
</style>

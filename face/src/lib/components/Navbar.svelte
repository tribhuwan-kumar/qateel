<script lang="ts">
	import { onMount } from "svelte";
	import { get } from 'svelte/store';
	import { mode } from "mode-watcher";
  import { formatSpeed, formatBytes } from "$lib/utils";
  import { systemState } from '$lib/system';
	import { historySort, loadHistorySort } from "$lib/sort";
	import { type SortBy, type Order, type HistorySort } from '$lib/sort';
  import { Badge } from "$lib/components/ui/badge";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Button } from "$lib/components/ui/button";
	import AddDl from '$lib/components/AddDl.svelte';
  import * as Dialog from "$lib/components/ui/dialog";
	import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
	import { JsonView } from '@zerodevx/svelte-json-view'
  import { Label } from "$lib/components/ui/label/index.js";
  import { Checkbox } from "$lib/components/ui/checkbox/index.js";
	import * as SidebarUI from "$lib/components/ui/sidebar/index.js";
  import SearchDialog from "$lib/components/SearchDialog.svelte";
  import {
    aria2,
		aria2Store,
  } from '$lib/aria2/client.svelte';
	import { selectedCategory, selectedStatus } from "$lib/categorize";
	import {
		selectedGids,
			selectedTotalBytes,
			clearSelection,
			setSelection,
	} from '$lib/selection';
	import {
		CodeIcon,
    PlayIcon,
    ListXIcon,
    PauseIcon,
		AlphaIcon,
		Time04Icon,
		Menu04Icon,
    Search02Icon,
    Cancel02Icon,
		Select02Icon,
    Delete02Icon,
		PlusSignIcon,
		Sorting05Icon,
		FloppyDiskIcon,
		CancelSquareIcon,
    ArrowUpDoubleIcon,
    ArrowDownDoubleIcon,
    CheckmarkBadge03Icon,
		CircleArrowReload02Icon,
  } from '@hugeicons/core-free-icons';
	import Up from "../../assets/svgs/up.svg";
	import Down from "../../assets/svgs/down.svg";
	import UpLight from "../../assets/svgs/up-light.svg";
	import DownLight from "../../assets/svgs/down-light.svg";

	let isDarkMode = $state(false);
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

  let sortMenuRef: HTMLDivElement | null = null;
  let sortMenuOpen = $state(false);
	let tempSortBy = $state<SortBy>("time");
	let tempOrder = $state<Order>("desc");

  const sortOptions: Array<{ key: SortBy; label: string }> = [
    { key: "time", label: "Time" },
    { key: "name", label: "Name" },
    { key: "size", label: "Size" },
  ];

  const globalStatLabels: Record<string, string> = {
		downloadSpeed: "Download speed",
		uploadSpeed: "Upload speed",
		numActive: "Active",
		numStopped: "Stopped",
		numStoppedTotal: "Stopped total",
		numWaiting: "Waiting"
  };

  function handleAdd() {
    addDialogOpen = true;
  }

  // Only `paused` gets resumed
  async function handleResume() {
    const targets = selectedItems.filter(i => ['paused'].includes(i.status));
		const gids = targets.map(i => (i.gid));
		console.log("Resuming gids:", gids);
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
	// 'active', 'seeding', 'waiting', 'paused', 'error' can be stopped
  async function handleStop() {
    const targets = selectedItems.filter(i => ['active', 'seeding', 'waiting', 'paused', 'error'].includes(i.status));
		const gids = targets.map(i => (i.gid));
		await aria2.stop(gids);
    clearSelection();
  }

  /// Removed status means file allocation deleted
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

  function toggleOrder() {
    tempOrder = tempOrder === "asc" ? "desc" : "asc";
    historySort.set({
      sortBy: tempSortBy,
      order: tempOrder,
    });
		console.log("History sort:", get(historySort));
    sortMenuOpen = false;
  }

  function selectSort(next: SortBy) {
    tempSortBy = next;
    historySort.set({
      sortBy: tempSortBy,
      order: tempOrder,
    });
		console.log("History sort:", get(historySort));
    sortMenuOpen = false;
  }

	async function handleSelectAll() {
		if (hasSelection) {
			clearSelection();
			return;
		}
		// If local items already contain everything, use them immediately
		if (aria2Store.items.length > 0) {
			const allItems = await aria2.getAllHistory($selectedStatus, $selectedCategory);
			if (allItems.length > 0) {
				setSelection(allItems.map(i => ({ gid: i.gid, totalLength: i.totalLength})));
			} else {
				setSelection(aria2Store.items.map(i => ({ gid: i.gid, totalLength: i.totalLength })));
			}
		}
	}

  function getVisibleOrderAsset() {
    // keep your required behavior:
    // asc => down icon visible
    // desc => up icon visible
    if (tempOrder === "asc") {
      return isDarkMode ? DownLight : Down;
    }

    return isDarkMode ? UpLight : Up;
  }

  $effect(() => {
		const savedMode = localStorage.getItem("mode-watcher-mode");
		if (mode.current === "dark" || savedMode === "dark") {
			isDarkMode = true;
		} else {
			isDarkMode = false;
		}

		console.log("Aria2 alive:", isAria2Alive);
  });

  function syncTheme() {
    const savedMode = localStorage.getItem("mode-watcher-mode");
    isDarkMode = mode.current === "dark" || savedMode === "dark";
  }

  function syncFromStore(value: HistorySort) {
    tempSortBy = value.sortBy;
    tempOrder = value.order;
  }

  onMount(() => {
    syncTheme();
    const saved = loadHistorySort();
    syncFromStore(saved);
    historySort.set(saved);

    const handleOutsideClick = (event: MouseEvent) => {
      if (!sortMenuRef) return;
      if (!sortMenuRef.contains(event.target as Node)) {
        sortMenuOpen = false;
      }
    };

    const unsubscribe = historySort.subscribe((value) => {
      syncFromStore(value);
    });

    document.addEventListener("pointerdown", handleOutsideClick);

    return () => {
      unsubscribe();
      document.removeEventListener("pointerdown", handleOutsideClick);
    };
  });
</script>


<div class="bg-background/95 backdrop-blur z-10 supports-backdrop-filter:bg-background/90
	dark:supports-backdrop-filter:bg-background/60 sticky top-0 w-full"
>
	<div class="inline-flex h-full items-center flex-row">
		<SidebarUI.Trigger
			class={`m-2 sm:hidden text-muted-foreground bg-background/95 backdrop-blur
				dark:supports-backdrop-filter:bg-background/60 supports-backdrop-filter:bg-background/90 sticky top-0
				cursor-pointer ${hasSelection ? "hidden" : ""}`}
		/>
		<SidebarUI.Trigger
			class={`m-2 max-sm:hidden text-muted-foreground bg-background/95 backdrop-blur
				dark:supports-backdrop-filter:bg-background/60 supports-backdrop-filter:bg-background/90 sticky top-0
				cursor-pointer`}
		/>
		<!-- For desktop -->
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

		<!-- For mobile -->
		<div
			class={`sm:hidden justify-start float-left md:h-12 items-center md:px-4 gap-2 ${hasSelection ? "hidden" : ""}`}
		>
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

  <div class="inline-flex justify-end float-right h-12 items-center px-1 gap-1 sm:px-4 sm:gap-2">
    <div class="flex items-center text-sm text-muted-foreground">
      {#if hasSelection}
        <div class="flex flex-row items-center max-sm:w-fit max-h-[80%] text-primary/90 sm:gap-2 border border-muted-foreground/20 sm:px-2 py-1
					rounded-full px-1 pl-2 animate-in fade-in zoom-in duration-200"
				>
					<span>
						<HugeiconsIcon
							icon={CheckmarkBadge03Icon}
							size={18}
							color="currentColor"
							strokeWidth={1.5}
							class="md:w-4.5 w-4 max-sm:hidden"
						/>
					</span>
					<span class="flex flex-col">
						<span
							class="text-[12px] sm:text-xs max-sm:pl-1"
						>
							{count}&nbsp;&nbsp;Selected
						</span>
						{#if $selectedTotalBytes > 0}
							{const totalLength = formatBytes(String($selectedTotalBytes))}
							<span class="opacity-75 font-normal text-[11px] sm:text-xs">
								({totalLength})
							</span>
						{/if}
					</span>
          <button
            onclick={clearSelection}
            class="md:ml-1 hover:bg-muted-foreground/20 rounded-full p-0.5 cursor-pointer"
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
        <div class="w-px h-6 mx-2 bg-border max-sm:hidden"></div>
      {/if}
    </div>

    <Button
      onclick={handleSearch}
      disabled={!isAria2Alive}
      class="cursor-pointer! disabled:cursor-not-allowed! max-sm:hidden"
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
      onclick={handleSearch}
      disabled={!isAria2Alive}
      class={`cursor-pointer! disabled:cursor-not-allowed! sm:hidden ${hasSelection ? "hidden" : ""}`}
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

		<!-- For desktop -->
		<Button
			onclick={handleSelectAll}
			disabled={!isAria2Alive}
			class="cursor-pointer! disabled:cursor-not-allowed! max-sm:hidden"
			size="icon"
			variant={hasSelection ? "secondary" : "ghost"}
			title={hasSelection ? "Deselect all" : "Select all (Ctrl+A)"}
		>
			<HugeiconsIcon
				icon={Select02Icon}
				class={hasSelection ? "text-primary w-4! h-4!" : "text-muted-foreground w-4! h-4!"}
				strokeWidth={2}
			/>
		</Button>

		<!-- For mobile -->
		<Button
			onclick={handleSelectAll}
			disabled={!isAria2Alive}
			class={`cursor-pointer! disabled:cursor-not-allowed! sm:hidden ${hasSelection ? "hidden" : ""}`}
			size="icon"
			variant={hasSelection ? "secondary" : "ghost"}
			title={hasSelection ? "Deselect all" : "Select all (Ctrl+A)"}
		>
			<HugeiconsIcon
				icon={Select02Icon}
				class={hasSelection ? "text-primary w-4! h-4!" : "text-muted-foreground w-4! h-4!"}
				strokeWidth={2}
			/>
		</Button>

		<!-- For desktop -->
		<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
    <div
			title="Sort by"
			bind:this={sortMenuRef}
			onclick={() => (sortMenuOpen = !sortMenuOpen)}
      class={["cursor-pointer! p-2 rounded-full disabled:cursor-not-allowed! relative hover:bg-muted hover:text-foreground dark:hover:bg-muted/50 aria-expanded:bg-muted aria-expanded:text-foreground", hasSelection && "max-sm:hidden"]}
    >
      <HugeiconsIcon
        icon={Sorting05Icon}
        class="text-muted-foreground w-4! h-4!"
        strokeWidth={2}
      />
			{#if sortMenuOpen}
				<div
					class="py-2 gap-x-2 px-2 top-10 text-foreground/95 text-[14px] z-10 inline-flex flex-row justify-center absolute rounded-xl border border-sidebar-border/80 bg-background/95 dark:bg-background/90
						supports-backdrop-filter:bg-background/90 dark:supports-backdrop-filter:bg-background/90 backdrop-blur-3xl"
					style="left: 50%; transform: translate(-50%, 0%);  width: 150px;"
				>
					<div class="w-[75%] gap-2 flex flex-col">
						{#each sortOptions as option (option)}
							<button
								type="button"
								onclick={() => selectSort(option.key)}
								class={[
									"flex h-full p-1.5 w-full cursor-pointer flex-row gap-2 items-center rounded-xl transition-colors",
									tempSortBy === option.key ? "highlight-field" : ""
								].join(" ")}
							>
								<HugeiconsIcon
									icon={option.key === "time" ? Time04Icon : option.key === "name" ? AlphaIcon : FloppyDiskIcon}
									class="text-foreground/95 w-4! h-4!"
									strokeWidth={2}
								/>
								<span>{option.label}</span>
							</button>
						{/each}
					</div>
					<div
						class="h-[inherit]! w-[25%] px-1 flex items-center highlight-field rounded-xl"
						title="Toggle ascending or descending"
					>
						<button
							title={tempOrder === "asc" ? "Descending" : "Ascending"}
							onclick={toggleOrder}
							aria-label={tempOrder === "asc" ? "Switch to descending" : "Switch to ascending"}
							class={["w-full h-full flex items-center justify-center cursor-pointer rounded-lg transition-all",].join(" ")}
						>
							<img
								aria-hidden="true"
								src={getVisibleOrderAsset()}
								alt={tempOrder === "asc" ? "switch to descending" : "switch to ascending"}
								class="w-full h-[100%] sm:w-7 xs:w-6 shrink-0 object-contain"
							/>
						</button>
					</div>
				</div>
			{/if}
    </div>

		<!-- vertical seperator -->
    <div class="w-px h-6 bg-border"></div>

		<!-- For desktop -->
    <Button
			disabled={!isAria2Alive}
      onclick={handleAdd}
      class="cursor-pointer disabled:cursor-not-allowed! max-sm:hidden"
			size="icon-sm"
      variant="ghost"
      title="Add downloads"
    >
      <HugeiconsIcon
        icon={PlusSignIcon}
        class="text-muted-foreground w-4! h-4!"
        strokeWidth={2}
      />
    </Button>

		<!-- For mobile -->
    <Button
			disabled={!isAria2Alive}
      onclick={handleAdd}
      class={`cursor-pointer disabled:cursor-not-allowed! sm:hidden ${hasSelection ? "hidden" : "flex"}`}
			size="icon-sm"
      variant="ghost"
      title="Add downloads"
    >
      <HugeiconsIcon
        icon={PlusSignIcon}
        class="text-muted-foreground w-4! h-4!"
        strokeWidth={2}
      />
    </Button>

    <AddDl bind:open={addDialogOpen} onclose={() => addDialogOpen = false} />
		<SearchDialog bind:open={searchDialogOpen} />

		<div class="w-px h-6 bg-border max-sm:hidden"></div>

		<!-- For mobile devices -->
		<div
			class={`items-center sm:hidden ${hasSelection ? "flex" : "hidden"}`}
		>
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
		</div>

		<!-- Desktop screen action buttons -->
    <div class="items-center gap-1 hidden sm:flex">
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
    </div>

		<!-- seperator -->
		<div class="w-px h-6 bg-border"></div>
		<DropdownMenu.Root>
			<DropdownMenu.Trigger
				title="More"
				class="cursor-pointer disabled:cursor-not-allowed! rounded-full hover:bg-muted-foreground/10"
				disabled={!isAria2Alive}
			>
				<HugeiconsIcon
					icon={Menu04Icon}
					class="text-muted-foreground w-4! h-4!"
					strokeWidth={2}
				/>
				<span class="sr-only">Open menu</span>
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end" class="sm:p-2 cursor-pointer text-xs sm:text-sm">
				<DropdownMenu.Item
					title="Force pause all active, waiting downloads"
					class="flex hover:rounded-xl items-center px-2 py-1.5 gap-x-1 rounded-md
						pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
					onclick={async () => {
						await aria2.pauseAll();
					}}
				>
					<HugeiconsIcon
						icon={PauseIcon}
						class="w-4! h-4!"
						strokeWidth={2}
					/>
					Pause all
				</DropdownMenu.Item>
				<DropdownMenu.Separator />
				<DropdownMenu.Item
					title="Resume all paused downloads"
					class="flex hover:rounded-xl items-center px-2 py-1.5 gap-x-1 rounded-md
						pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
					onclick={async () => {
						await aria2.resumeAll();
					}}
				>
					<HugeiconsIcon
						icon={PlayIcon}
						class="w-4! h-4!"
						strokeWidth={2}
					/>
					Resume all
				</DropdownMenu.Item>
				<DropdownMenu.Separator />
				<DropdownMenu.Item
					title="Retry all error, stopped downloads"
					class="flex hover:rounded-xl items-center px-2 py-1.5 gap-x-1 rounded-md
						pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
					onclick={async () => {
						await aria2.retryAll();
					}}
				>
					<HugeiconsIcon
						icon={CircleArrowReload02Icon}
						class="w-4! h-4!"
						strokeWidth={2}
					/>
					Retry all
				</DropdownMenu.Item>
				<DropdownMenu.Separator />
				<DropdownMenu.Item
					title="Stop all complete, error, stopped downloads"
					class="flex hover:rounded-xl items-center px-2 py-1.5 gap-x-1 rounded-md
						pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
					onclick={async () => {
						await aria2.stopAll();
					}}
				>
					<HugeiconsIcon
						icon={ListXIcon}
						class="w-4! h-4!"
						strokeWidth={2}
					/>
					Stop all
				</DropdownMenu.Item>
				<DropdownMenu.Separator />
				<DropdownMenu.Item
					title="Purge all complete, error, removed downloads"
					class="flex hover:rounded-xl items-center px-2 py-1.5 gap-x-1 rounded-md
						pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
					onclick={async () => {
						await aria2.purgeAll();
					}}
				>
					<HugeiconsIcon
						icon={CancelSquareIcon}
						class="w-4! h-4!"
						strokeWidth={2}
					/>
					Purge all
				</DropdownMenu.Item>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
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
				<div class="min-h-[200px] overflow-x-scroll overflow-y-scroll z-100 json-class text-xs sm:text-sm overflow-auto rounded-xl border bg-muted/40 p-4">
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


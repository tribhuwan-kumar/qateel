<script lang="ts">
	import { toast } from "svelte-sonner";
	import { aria2 } from "$lib/aria2/client.svelte";
	import { Button } from "$lib/components/ui/button";
	import {
		Dialog,
		DialogTitle,
		DialogFooter,
		DialogHeader,
		DialogContent,
		DialogDescription
	} from "$lib/components/ui/dialog";
	import { HugeiconsIcon } from "@hugeicons/svelte";
	import {
		Checkmark,
		Alert01Icon,
		Cancel01Icon,
		Download01Icon,
		Settings01Icon,
		LoaderPinwheelIcon,
		HardDriveIcon
	} from "@hugeicons/core-free-icons";
	import { formatBytes } from "$lib/utils";
	import { getDownloadIconName, downloadIcons } from "$lib/categorize";
	import type {
		AddUriReq,
		UriInfoRes,
		AddUriTask,
		ItemMetaData,
		Aria2Options,
		DuplicateAction,
	} from "$lib/aria2/types";
	import OptionsDialog from "$lib/components/settings/Options.svelte";

	let {
		open = $bindable(false),
		uris = [],
		onclose
	} = $props<{ open: boolean; uris: string[]; onclose: () => void }>();

	type UIItem = UriInfoRes & {
		uiStatus: "pendingConflict" | "ready" | "downloading" | "done" | "fatalError";
		selectedAction?: DuplicateAction;
		newName: string;
		options: Partial<Aria2Options>;
	};

	let items = $state<UIItem[]>([]);
	let isFetching = $state(false);
	let isSubmitting = $state(false);
	let nameCheckTimers: Record<number, ReturnType<typeof setTimeout>> = {};
	let editingItemIndex = $state<number | null>(null);

	let hasUnresolvedConflicts = $derived(items.some((i) => i.uiStatus === "pendingConflict"));
	let allDoneOrFatal = $derived(
		items.length > 0 && items.every((i) => i.uiStatus === "done" || i.uiStatus === "fatalError")
	);

	$effect(() => {
		if (open && uris.length > 0) fetchPreflight();
		else if (!open) items = [];
	});

	async function fetchPreflight() {
		isFetching = true;
		try {
			const res = await aria2.probeUris(uris);
			if (res) {
				items = res.map((r) => ({
					...r,
					newName: r.name,
					uiStatus: r.probeError ? "fatalError" : r.isDuplicate ? "pendingConflict" : "ready",
					options: {}
				}));
			}
		} finally {
			isFetching = false;
		}
	}

	function resolveConflict(index: number, action: DuplicateAction) {
		items[index].selectedAction = action;
		items[index].uiStatus = "ready";
	}

	function undoConflictResolution(index: number) {
		items[index].selectedAction = undefined;
		items[index].uiStatus = "pendingConflict";
	}

	function handleNameEdit(index: number) {
		const item = items[index];
		if (item.uiStatus === "fatalError" || item.isMagnet) return;

		item.selectedAction = undefined;

		if (nameCheckTimers[index]) clearTimeout(nameCheckTimers[index]);

		nameCheckTimers[index] = setTimeout(async () => {
			const res = await aria2.checkName(item.newName);
			if (res.isConflict) {
				item.uiStatus = "pendingConflict";
				item.conflictMsg = res.msg || "Name conflict detected.";
			} else {
				item.uiStatus = "ready";
				item.conflictMsg = null;
			}
		}, 400);
	}

	function buildTask(item: UIItem): AddUriTask {
		return {
			uri: item.uri,
			name: item.newName,
			totalLength: item.totalLength,
			isResumeSupported: item.isResumeSupported,
			duplicateAction: item.selectedAction,
			options: {}
		};
	}

	function getCategoryIcons(item: UIItem) {
		const mockMeta = {
			name: item.newName,
			kind: item.isMagnet ? "torrent" : "uri",
			torrent: item.isMagnet ? {} : null
		} as unknown as ItemMetaData;

		const iconKey = getDownloadIconName(mockMeta) as keyof typeof downloadIcons;
		return downloadIcons[iconKey] || downloadIcons.everything;
	}

	async function downloadSingle(index: number) {
		const item = items[index];
		if (
			item.uiStatus === "pendingConflict" ||
			item.uiStatus === "done" ||
			item.uiStatus === "fatalError"
		)
			return;

		item.uiStatus = "downloading";
		try {
			const uriReq: AddUriReq = {
					tasks: [buildTask(item)]
			};
			const results = await aria2.addUris(uriReq);
			item.uiStatus = results && results[0] && !results[0].error ? "done" : "fatalError";
		} catch {
			item.uiStatus = "fatalError";
		}

		if (items.every((i) => i.uiStatus === "done" || i.uiStatus === "fatalError"))
			setTimeout(onclose, 1500);
	}

	async function downloadAll() {
		if (hasUnresolvedConflicts) return;

		isSubmitting = true;
		const validItems = items.filter((i) => i.uiStatus === "ready");
		const skippedItems = items.filter((i) => i.uiStatus === "fatalError").length;

		validItems.forEach((i) => (i.uiStatus = "downloading"));

		if (validItems.length > 0) {
			try {
			const urisReq: AddUriReq = {
					tasks: validItems.map(buildTask)
			};
				const results = await aria2.addUris(urisReq);
				if (results) {
					validItems.forEach((item, i) => {
						const res = results.find((r: any) => r.index === i);
						item.uiStatus = res && !res.error ? "done" : "fatalError";
					});
				}
			} catch {
				validItems.forEach((i) => (i.uiStatus = "fatalError"));
			}
		}

		if (skippedItems > 0) {
			toast.warning(`
				Skipped ${skippedItems} unreachable or invalid URIs.`, {
				richColors: true,
				style: "cursor: pointer;"
			});
		}

		isSubmitting = false;
		setTimeout(onclose, 1500);
	}

	function openSettings(index: number) {
    editingItemIndex = index;
  }

  function saveOptions(newOptions: Partial<Aria2Options>) {
    if (editingItemIndex !== null) {
      items[editingItemIndex].options = newOptions;
    }
  }
</script>

<Dialog bind:open onOpenChange={(v) => !v && onclose()}>
	<DialogContent
		class="sm:max-w-4xl h-[85vh] flex flex-col p-4 gap-0 overflow-hidden"
		showCloseButton={false}
	>
		<DialogHeader class="mb-4 mt-4 ml-4 shrink-0">
			<DialogTitle>Review download queue</DialogTitle>
			<DialogDescription>
				Verify files & resolve any conflicts before adding in aria2 queue...
			</DialogDescription>
		</DialogHeader>

		<div class="flex-1 overflow-y-auto space-y-3 pr-2 min-h-0 pb-16">
			<!-- Invisible focus anchor to absorb modal focus trap resets -->
			<div tabindex="0" class="sr-only" aria-hidden="true"></div>

			{#if isFetching}
				<div
					class="flex flex-col items-center justify-center h-full space-y-4 text-muted-foreground min-h-[300px]"
				>
					<HugeiconsIcon icon={LoaderPinwheelIcon} class="w-8 h-8 animate-spin" strokeWidth={2} />
					<p class="text-sm animate-pulse">Probing servers and resolving metadata...</p>
				</div>
			{:else}
				{#each items as item, index (index)}
					{@const catIcons = getCategoryIcons(item)}

					<div
						class="flex flex-col p-3 border rounded-lg transition-colors
            {item.uiStatus === 'fatalError'
							? 'border-destructive/40 bg-destructive/5'
							: 'border-[#DDA0DD]/60 bg-[#DDA0DD]/5'}"
					>
						<div class="flex items-center gap-3 w-full">
							<!-- Categorized Visual Icon -->
							<div class="w-8 h-8 shrink-0 flex items-center justify-center">
								<img
									src={catIcons.light}
									alt="icon"
									class="w-7 h-7 dark:hidden"
								/>
								<img
									src={catIcons.dark}
									alt="icon"
									class="w-6 h-6 hidden dark:block"
								/>
							</div>

							<!-- Name input -->
							<div class="flex-1 min-w-0 relative">
								<input
									type="text"
									bind:value={item.newName}
									oninput={() => handleNameEdit(index)}
									title={item.isMagnet ? "Magnets can't be renamed before downloading" : ""}
									disabled={item.uiStatus === "downloading" ||
										item.uiStatus === "done" ||
										item.uiStatus === "fatalError" ||
										item.isMagnet}
									class="w-full text-[13px] font-medium bg-transparent border-b border-transparent
										hover:border-sidebar-border focus:border-primary focus:outline-none disabled:opacity-70
										disabled:hover:border-transparent transition-colors px-1 py-0.5 truncate"
								/>
							</div>

							<!-- Individual action button -->
							<div class="shrink-0 w-10 flex justify-end">
								{#if item.uiStatus === "done"}
									<span
										title={item.uiStatus === "done" ?
											"Added in download queue!!" : ""}
									>
										<HugeiconsIcon
											icon={Checkmark}
											class="w-5 h-5 text-muted-foreground"
										/>
									</span>
								{:else if item.uiStatus === "downloading"}
									<HugeiconsIcon
										icon={LoaderPinwheelIcon}
										class="w-5 h-5 animate-spin text-muted-foreground"
									/>
								{:else if item.uiStatus !== "fatalError"}
									<div class="shrink-0 flex justify-end gap-1">
										<Button
											variant="outline"
											size="icon"
											class="h-7 w-7 disabled:cursor-not-allowed cursor-pointer"
											onclick={() => openSettings(index)}
										>
											<HugeiconsIcon
												icon={Settings01Icon}
												class="w-4 h-4"
											/>
										</Button>
										<Button
											title={item.uiStatus === "pendingConflict" ?
												"Please resolve conflict either either by `Overwrite` or `Download as copy`" : ""}
											variant="outline"
											size="icon"
											class="h-7 w-7 disabled:cursor-not-allowed cursor-pointer"
											disabled={item.uiStatus === "pendingConflict"}
											onclick={() => downloadSingle(index)}
										>
											<HugeiconsIcon
												icon={Download01Icon}
												class="w-4 h-4"
											/>
										</Button>

									</div>
								{/if}
							</div>
						</div>

						<!-- Error & conflict resolution area -->
						<div class="flex flex-row mt-2 ml-11 items-center gap-4">
							{#if item.uiStatus === "fatalError"}
								<div
									class="flex items-center gap-1.5 text-[11px] text-destructive font-medium"
								>
									<HugeiconsIcon icon={Alert01Icon} class="w-3.5 h-3.5" />
									<span>
										{item.probeError || "Invalid or unreachable URL"}
									</span>
								</div>
							{:else if item.uiStatus === "pendingConflict"}
								<div class="flex items-center gap-3">
									<span class="text-[11px] text-muted-foreground/90 font-medium flex items-center gap-1">
										<HugeiconsIcon
											icon={Alert01Icon}
											class="w-3.5 h-3.5"
										/>
										{item.conflictMsg}
									</span>

									<div class="flex gap-2">
										<Button
											size="sm"
											variant="outline"
											class="h-6 text-[10px] px-3 cursor-pointer border-[#DDA0DD]/60"
											onclick={() => resolveConflict(index, "overwrite")}
										>
											Overwrite file
										</Button>
										{#if !item.isMagnet && item.conflictType !== "hash"}
											<Button
												size="sm"
												variant="outline"
												class="h-6 text-[10px] px-3 cursor-pointer border-[#DDA0DD]/60"
												onclick={() => resolveConflict(index, "copy")}
											>
												Download as copy
											</Button>
										{/if}
										{#if item.conflictType === 'hash' || item.isResumeSupported}
												<!-- Integrity check trigger -->
												<Button
													size="sm"
													variant="outline"
													class="h-6 text-[10px] px-3 cursor-pointer border-[#DDA0DD]/60"
												>

													<!-- onclick={() => handleIntegrityCheck(index)} -->
													Verify integrity
												</Button>
											{/if}
									</div>
								</div>
							{:else if item.selectedAction && item.uiStatus === "ready"}
								<div class="flex items-center font-medium gap-1 text-[11px]">
									<HugeiconsIcon
										icon={Checkmark}
										class="w-3.5 h-3.5 text-muted-foreground"
									/>
									<span class="text-muted-foreground font-medium!">
										{item.selectedAction === "overwrite"
											? "Will overwrite existing file!!"
											: "Will download as a new copy!!"}
									</span>
									<button
										class="underline italic cursor-pointer hover:text-foreground ml-2"
										onclick={() => undoConflictResolution(index)}
									>
										Change
									</button>
								</div>
							{/if}
							{#if item.uiStatus !== "fatalError"}
								{#if item.uiStatus !== "ready" && item.uiStatus !== "done"}
									<div class="h-5 w-[1px] rounded-xl bg-muted-foreground"></div>
								{/if}
								<div class="flex items-center justify-evenly gap-6 shrink-0 text-[11px] font-medium mr-2">
									<div class="flex items-center gap-1
										{item.isResumeSupported ? 'text-muted-foreground' : 'text-destructive/80'}"
									>
										<HugeiconsIcon
											icon={item.isResumeSupported ? Checkmark : Cancel01Icon}
											class="w-3.5 h-3.5"
										/>
										{item.isResumeSupported ? "Resumable" : "Not resumable"}
									</div>
									{#if item.totalLength !== "0"}
										<div class="flex items-center gap-1 text-muted-foreground">
											<HugeiconsIcon
												icon={HardDriveIcon}
												class="w-3.5 h-3.5"
											/>
												{formatBytes(item.totalLength)}
										</div>
									{/if}
								</div>
						{/if}
						</div>
					</div>
				{/each}
			{/if}
		</div>

		<DialogFooter class="pt-4 mt-2 max-sm:flex-col gap-6 shrink-0 z-10">
			<Button
				variant="outline"
				onclick={onclose}
				disabled={isFetching || isSubmitting}
				class="disabled:cursor-not-allowed cursor-pointer min-w-24"
			>
				{allDoneOrFatal ? "Close" : "Cancel"}
			</Button>
			<Button
				onclick={downloadAll}
				class="disabled:cursor-not-allowed cursor-pointer min-w-36"
				disabled={isFetching ||
					isSubmitting ||
					hasUnresolvedConflicts ||
					allDoneOrFatal ||
					items.length === 0}
			>
				{#if isSubmitting}
					<HugeiconsIcon
						icon={LoaderPinwheelIcon}
						class="w-4 h-4 animate-spin mr-2" />
						Adding...
				{:else}
					Download all
				{/if}
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

{#if editingItemIndex !== null}
  <OptionsDialog
    open={true}
    initialOptions={items[editingItemIndex].options}
    kind={items[editingItemIndex].isMagnet ? "torrent" : "uri"}
    filename={items[editingItemIndex].newName}
    onsave={saveOptions}
    onclose={() => editingItemIndex = null}
  />
{/if}



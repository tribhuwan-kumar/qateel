<script lang="ts">
	import { toast } from "svelte-sonner";
	import { truncateMiddle } from "$lib/utils";
	import { aria2, aria2Info } from "$lib/aria2/client.svelte";
	import { Button } from "$lib/components/ui/button";
	import * as Select from "$lib/components/ui/select";
  import * as Tooltip from "$lib/components/ui/tooltip";
	import * as PopoverTooltip from '$lib/components/ui/popovertooltip';
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
		File01Icon,
		Alert01Icon,
		Cancel01Icon,
		Delete01Icon,
		FileLinkIcon,
		ArrowUp01Icon,
		FloppyDiskIcon,
		Download03Icon,
		ArrowDown01Icon,
		LoaderPinwheelIcon,
		GitMergeConflictIcon,
		DashboardCircleSettingsIcon,
	} from "@hugeicons/core-free-icons";
	import { formatBytes } from "$lib/utils";
	import { getDownloadIconName, downloadIcons } from "$lib/categorize";
	import Label from "$lib/components/ui/label/label.svelte";
	import type {
		AddUriReq,
		UriInfoRes,
		AddUriTask,
		ItemMetaData,
		Aria2Options
	} from "$lib/aria2/types";
	import Virtual from '$lib/components/Virtual.svelte';
	import OptionsDialog from "$lib/components/settings/Options.svelte";

	type ConflictAction =
		| "overwrite"
		| "copy"
		| "verify"
		| "redownload"
		| "resume"
		| "movePosition"
		| "moveQueue"
		| "retry"
		| "stopSeeding";

	let {
		open = $bindable(false),
		uris = [],
		onclose
	} = $props<{
		open: boolean;
		uris: string[];
		onclose: () => void;
	}>();

	type UIItem = UriInfoRes & {
		uiStatus: "pendingConflict" | "ready" | "downloading" | "done" | "fatalError";
		newName: string;
		options: Partial<Aria2Options>;
		conflictOptions: Partial<Aria2Options>;
		selectedAction: ConflictAction | null;
		allowBypassConflict: boolean;
	};

	let items = $state<UIItem[]>([]);
	let isFetching = $state(false);
	let isSubmitting = $state(false);
	let nameCheckTimers: Record<number, ReturnType<typeof setTimeout>> = {};
	let editingItemIndex = $state<number | null>(null);
	let optionsDialogOpen = $state(false);
	let integrityDialogOpen = $state(false);
	let integrityDialogIndex = $state<number | null>(null);
	let integrityAlgorithm = $state("sha-256");
	let integrityHash = $state("");
	let integrityRequiresChecksum = $state(true);

	let hasUnresolvedConflicts = $derived(items.some((i) => i.uiStatus === "pendingConflict"));
	let conflictCount = $derived(items.filter((i) => i.uiStatus === "pendingConflict").length);
	let allDoneOrFatal = $derived(
		items.length > 0 && items.every((i) => i.uiStatus === "done" || i.uiStatus === "fatalError")
	);

	let hashAlgorithms = $derived(
		Array.isArray($aria2Info?.hashAlgorithms) ? $aria2Info.hashAlgorithms : ["sha-256", "md5"]
	);

	const actionLabels: Record<ConflictAction, string> = {
		overwrite: "Overwrite file",
		copy: "Download as copy",
		verify: "Verify integrity",
		redownload: "Redownload",
		resume: "Resume",
		movePosition: "Move position in queue",
		moveQueue: "Move to queue",
		retry: "Retry",
		stopSeeding: "Stop seeding"
	};

	$effect(() => {
		if (open && uris.length > 0) fetchPreflight();
		else if (!open) items = [];
	});

	$effect(() => {
		if (!optionsDialogOpen && editingItemIndex !== null) {
			editingItemIndex = null;
		}
	});

	function normalizeText(value: string | null | undefined) {
		return (value ?? "").trim().toLowerCase();
	}

	function isConflictMessage(item: UIItem, expected: string) {
		return normalizeText(item.conflict?.message) === normalizeText(expected);
	}

	function isPermissibleProbeError(message: string | null | undefined) {
		const normalized = normalizeText(message);
		return normalized === normalizeText("Magnet metadata resolution failed.") ||
	       normalized === normalizeText("Private torrent, set credentials in options.");
	}

	async function fetchPreflight() {
		isFetching = true;
		try {
			const res = await aria2.probeUris(uris);
			if (res) {
				items = res.map((r) => {
					const rules = getConflictButtonsAndRules(r as UIItem);
					const needsBlocking = rules.requiresResolution || rules.blocksDownload;
					const probeErrorIsFatal = !!r.probeError && !isPermissibleProbeError(r.probeError);

					return {
						...r,
						newName: r.name,
						uiStatus: probeErrorIsFatal
							? "fatalError"
							: r.isDuplicate && needsBlocking
								? "pendingConflict"
								: "ready",
						options: {},
						conflictOptions: {},
						selectedAction: null,
						allowBypassConflict: rules.allowBypass
					} as UIItem;
				});
			}
		} finally {
			isFetching = false;
		}
	}

	function removeItem(index: number) {
		items = items.filter((_, i) => i !== index);
		if (items.length === 0) onclose();
	}

	function resolveConflict(index: number, action: ConflictAction, checksum = "") {
		const item = items[index];
		item.selectedAction = action;
		item.conflictOptions = {
			...item.conflictOptions,
			...buildConflictOptions(action, checksum)
		};
		item.uiStatus = "ready";
	}

	function undoConflictResolution(index: number) {
		items[index].uiStatus = "pendingConflict";
		items[index].selectedAction = null;
		items[index].conflictOptions = {};
		if (items[index].options.checksum) {
			delete items[index].options.checksum;
		}
	}

	function handleNameEdit(index: number) {
		const item = items[index];
		if (item.uiStatus === "fatalError" || item.isMagnet) return;

		if (nameCheckTimers[index]) clearTimeout(nameCheckTimers[index]);

		nameCheckTimers[index] = setTimeout(async () => {
			const res = await aria2.checkName(item.newName);
			if (res.isConflict) {
				item.uiStatus = "pendingConflict";
				if (item.conflict && item.conflict.message) {
					item.conflict.message = res.msg || "Name conflict detected.";
				}
				if (item.conflict && item.conflict.conflictType) {
					item.conflict.conflictType = "file";
				}
				const rules = getConflictButtonsAndRules(item);
				item.allowBypassConflict = rules.allowBypass;
			} else {
				item.uiStatus = "ready";
				if (item.conflict && item.conflict.message) {
					item.conflict.message = null;
				}
			}
		}, 400);
	}

	function buildTask(item: UIItem): AddUriTask {
		return {
			uri: item.uri,
			name: item.newName,
			totalLength: item.totalLength,
			isResumeSupported: item.isResumeSupported,
			options: {
				...item.options,
				...item.conflictOptions
			}
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

	function buildConflictOptions(action: ConflictAction, checksum = ""): Partial<Aria2Options> {
		switch (action) {
			case "overwrite":
				return {
					"allow-overwrite": "true",
					"auto-file-renaming": "false"
				};
			case "copy":
				return {
					"allow-overwrite": "false",
					"auto-file-renaming": "true"
				};
			case "verify":
				return {
					"check-integrity": "true",
					"auto-file-renaming": "false",
					...(checksum ? { checksum } : {})
				};
			default:
				return {};
		}
	}

	function getSelectedActionLabel(action: ConflictAction | null) {
		switch (action) {
			case "overwrite":
				return "Will Overwrite existing file.";
			case "verify":
				return "Will verifying file existing file integrity.";
			case "copy":
				return "Will Download as new copy.";
			case "redownload":
				return "Will redownload.";
			case "resume":
				return "Will resume existing task.";
			case "movePosition":
				return "Will move position in download queue.";
			case "moveQueue":
				return "Will move to download queue";
			case "retry":
				return "Will retry existing task.";
			case "stopSeeding":
				return "Will stopping seeding.";
			default:
				return "";
		}
	}

	async function performItemAction(index: number) {
		const item = items[index];

		if (item.uiStatus === "done" || item.uiStatus === "fatalError") return;
		if (item.uiStatus === "pendingConflict" && !item.selectedAction) return;

		const action = item.selectedAction;

		if (!action || action === "overwrite" || action === "copy" || action === "verify") {
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
		} else {
			if (!item.conflict?.gid) {
				item.uiStatus = "fatalError";
				return;
			}

			item.uiStatus = "downloading";
			try {
				switch (action) {
					case "redownload":
						// await aria2.redownload(item.existingGid);
						break;
					case "resume":
						await aria2.resume([item.conflict?.gid]);
						break;
					case "movePosition":
						// await aria2.movePosInQueue(item.existingGid);
						break;
					case "moveQueue":
						// await aria2.moveQueue(item.existingGid);
						break;
					case "retry":
						// await aria2.redownload(item.existingGid);
						break;
					case "stopSeeding":
						// await aria2.stopSeeding(item.existingGid);
						break;
				}
				item.uiStatus = "done";
			} catch {
				item.uiStatus = "fatalError";
			}
		}
	}

	async function downloadSingle(index: number) {
		await performItemAction(index);

		if (items.every((i) => i.uiStatus === "done" || i.uiStatus === "fatalError")) {
			setTimeout(onclose, 1500);
		}
	}

	async function downloadAll() {
		if (hasUnresolvedConflicts) return;

		isSubmitting = true;
		const readyItems = items.filter((i) => i.uiStatus === "ready");
		const skippedItems = items.filter((i) => i.uiStatus === "fatalError").length;

		if (readyItems.length > 0) {
			try {
				const tasks: AddUriTask[] = readyItems.map((item) => buildTask(item));
				const uriReq: AddUriReq = { tasks };
				const results = await aria2.addUris(uriReq);

				if (results && results.length > 0) {
					readyItems.forEach((item, idx) => {
						const itemIndex = items.indexOf(item);
						if (itemIndex !== -1) {
							items[itemIndex].uiStatus =
								results[idx] && !results[idx].error ? "done" : "fatalError";
						}
					});
				}
			} catch (e) {
				readyItems.forEach((item) => {
					const itemIndex = items.indexOf(item);
					if (itemIndex !== -1) {
						items[itemIndex].uiStatus = "fatalError";
					}
				});
			}
		}

		if (skippedItems > 0) {
			toast.warning(`Skipped ${skippedItems} unreachable or invalid URIs.`, {
				richColors: true,
				style: "cursor: pointer;"
			});
		}

		isSubmitting = false;
		setTimeout(onclose, 1500);
	}

	function openOptionsDialog(index: number) {
		editingItemIndex = index;
		optionsDialogOpen = true;
	}

	function saveOptions(newOptions: Partial<Aria2Options>) {
		if (editingItemIndex !== null) {
			items[editingItemIndex].options = {
				...items[editingItemIndex].options,
				...newOptions
			};
		}
	}

	function openIntegrityDialog(index: number) {
		integrityDialogIndex = index;
		integrityAlgorithm = hashAlgorithms[0] || "";
		integrityHash = "";
		integrityRequiresChecksum = !items[index].isMagnet;
		integrityDialogOpen = true;
	}

	function closeIntegrityDialog() {
		integrityDialogOpen = false;
		setTimeout(() => {
			integrityDialogIndex = null;
			integrityAlgorithm = "";
			integrityHash = "";
			integrityRequiresChecksum = true;
		}, 200);
	}

	function confirmIntegrityDialog() {
		if (integrityDialogIndex === null) return;
		const checksum = integrityRequiresChecksum ? `${integrityAlgorithm}=${integrityHash.trim()}` : "";
		resolveConflict(integrityDialogIndex, "verify", checksum);
		closeIntegrityDialog();
	}

	async function handleConflictAction(index: number, action: ConflictAction) {
		const item = items[index];
		console.log("item.conflict?.gid:", item.conflict?.gid)
		switch (action) {
			case "overwrite":
			case "copy":
				if (item.conflict?.gid || item.conflict?.conflictType === "untrackedFile") {
					item.selectedAction = action;
					item.conflictOptions = buildConflictOptions(action);
					item.uiStatus = "ready";
				}
			return;

			case "verify":
				if (item.isMagnet && item.conflict?.gid) {
					item.selectedAction = action;
					item.conflictOptions = buildConflictOptions(action);
					item.uiStatus = "ready";
					return;
				}

				openIntegrityDialog(index);
				return;

			case "redownload":
				if (item.conflict?.gid) {
					item.selectedAction = action;
					item.conflictOptions = buildConflictOptions(action);
					item.uiStatus = "done";
				}
				return;

			case "resume":
				if (item.conflict?.gid) {
					item.selectedAction = action;
					item.conflictOptions = buildConflictOptions(action);
					item.uiStatus = "done";
				}
				return;

			case "movePosition":
				if (item.conflict?.gid) {
					item.selectedAction = action;
					item.conflictOptions = buildConflictOptions(action);
					item.uiStatus = "done";
				}
				return;

			case "moveQueue":
				if (item.conflict?.gid) {
					item.selectedAction = action;
					item.conflictOptions = buildConflictOptions(action);
					item.uiStatus = "done";
				}
				return;

			case "stopSeeding":
				if (item.conflict?.gid) {
					item.selectedAction = action;
					item.conflictOptions = buildConflictOptions(action);
					item.uiStatus = "done";
				}
				return;

			case "retry":
				if (item.conflict?.gid) {
					item.selectedAction = action;
					item.conflictOptions = buildConflictOptions(action);
					item.uiStatus = "done";
				}
				return;
		}
	}

	function getConflictButtonsAndRules(item: UIItem) {
		const conflictType = item.conflict?.conflictType;
		const status = item.conflict?.status;

		if (!item.isDuplicate) {
			return {
				buttons: [],
				requiresResolution: false,
				blocksDownload: false,
				allowBypass: true
			};
		}

		if (conflictType === "file" || conflictType === "untrackedFile") {
			return {
				buttons: [
					{ action: "overwrite" as ConflictAction, label: actionLabels.overwrite },
					{ action: "copy" as ConflictAction, label: actionLabels.copy }
				],
				requiresResolution: true,
				blocksDownload: true,
				allowBypass: false
			};
		}

		if (conflictType === "hash") {
			if (isConflictMessage(item, "Same BitTorrent info hash and downloaded content already exists.")) {
				return {
					buttons: [
						{ action: "overwrite" as ConflictAction, label: actionLabels.overwrite },
						{ action: "copy" as ConflictAction, label: actionLabels.copy }
					],
					requiresResolution: true,
					blocksDownload: true,
					allowBypass: false
				};
			}

			if (
				isConflictMessage(
					item,
					"Same BitTorrent info hash already exists in database but the downloaded content doesn't exist."
				)
			) {
				return {
					buttons: [{ action: "redownload" as ConflictAction, label: actionLabels.redownload }],
					requiresResolution: false,
					blocksDownload: false,
					allowBypass: true
				};
			}

			switch (status) {
				case "active":
					return {
						buttons: [],
						requiresResolution: false,
						blocksDownload: true,
						allowBypass: false
					};
				case "paused":
					return {
						buttons: [{ action: "resume" as ConflictAction, label: actionLabels.resume }],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
				case "waiting":
					return {
						buttons: [{ action: "movePosition" as ConflictAction, label: actionLabels.movePosition }],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
				case "seeding":
					return {
						buttons: [{ action: "stopSeeding" as ConflictAction, label: actionLabels.stopSeeding }],
						requiresResolution: false,
						blocksDownload: true,
						allowBypass: false
					};
				case "stopped":
					return {
						buttons: [{ action: "moveQueue" as ConflictAction, label: actionLabels.moveQueue }],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
				case "error":
					return {
						buttons: [{ action: "retry" as ConflictAction, label: actionLabels.retry }],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
				default:
					return {
						buttons: [],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
			}
		}

		if (conflictType === "uri") {
			if (isConflictMessage(item, "Downloaded content from this URI already exists.")) {
				return {
					buttons: [
						{ action: "overwrite" as ConflictAction, label: actionLabels.overwrite },
						{ action: "copy" as ConflictAction, label: actionLabels.copy },
						{ action: "verify" as ConflictAction, label: actionLabels.verify }
					],
					requiresResolution: true,
					blocksDownload: true,
					allowBypass: false
				};
			}

			if (isConflictMessage(item, "Same URI exists in database but the downloaded content doesn't exist.")) {
				return {
					buttons: [{ action: "redownload" as ConflictAction, label: actionLabels.redownload }],
					requiresResolution: false,
					blocksDownload: false,
					allowBypass: true
				};
			}

			switch (status) {
				case "active":
					return {
						buttons: [],
						requiresResolution: false,
						blocksDownload: true,
						allowBypass: false
					};
				case "paused":
					return {
						buttons: [{ action: "resume" as ConflictAction, label: actionLabels.resume }],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
				case "waiting":
					return {
						buttons: [{ action: "movePosition" as ConflictAction, label: actionLabels.movePosition }],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
				case "stopped":
					return {
						buttons: [{ action: "moveQueue" as ConflictAction, label: actionLabels.moveQueue }],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
				case "error":
					return {
						buttons: [{ action: "retry" as ConflictAction, label: actionLabels.retry }],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
				default:
					return {
						buttons: [],
						requiresResolution: false,
						blocksDownload: false,
						allowBypass: true
					};
			}
		}

		return {
			buttons: [],
			requiresResolution: false,
			blocksDownload: false,
			allowBypass: true
		};
	}

	let virtualRef = $state<{ scrollToIndex: (index: number, behavior?: ScrollBehavior) => void } | null>(null);
	let virtualViewportEl = $state<HTMLDivElement | null>(null);
	let activeConflictIndex = $state<number | null>(null);
	let highlightedIndex = $state<number | null>(null);
	let highlightTimer: ReturnType<typeof setTimeout> | null = null;

	const conflictIndices = $derived(
		items
			.map((item, index) => ({ item, index }))
			.filter(({ item }) => item.uiStatus === "pendingConflict")
			.map(({ index }) => index));

	function getCurrentVisibleIndex(): number {
		if (!virtualViewportEl) return 0;
		const viewportRect = virtualViewportEl.getBoundingClientRect();
		const rows = virtualViewportEl.querySelectorAll<HTMLElement>("[data-index]");
		for (const row of rows) {
			const rect = row.getBoundingClientRect();
			if (rect.bottom > viewportRect.top + 30 && rect.top < viewportRect.top + 180) {
				const idx = parseInt(row.getAttribute("data-index") || "", 10);
				if (!isNaN(idx)) return idx;
			}
		}
		const stride = 108 + 8;
		return Math.floor((virtualViewportEl.scrollTop + 40) / stride);
	}

	function scrollToConflict(targetIndex: number) {
		activeConflictIndex = targetIndex;
    highlightedIndex = targetIndex;

		if (virtualRef) {
			virtualRef.scrollToIndex(targetIndex);
		} else if (virtualViewportEl) {
			const targetEl = virtualViewportEl.querySelector(`[data-index="${targetIndex}"]`) as HTMLElement | null;
			if (targetEl) {
				const targetTop = targetEl.offsetTop - (virtualViewportEl.clientHeight - targetEl.offsetHeight) / 2;
				virtualViewportEl.scrollTo({ top: Math.max(0, targetTop), behavior: "smooth" });
			}
		}

		const el = virtualViewportEl?.querySelector(`[data-index="${targetIndex}"]`) as HTMLElement | null;
		if (el) {
				el.classList.add("highlight-field");
		}

		if (highlightTimer) clearTimeout(highlightTimer);

		highlightTimer = setTimeout(() => {
			if (highlightedIndex === targetIndex) {
					highlightedIndex = null;
			}
      const currentEl = virtualViewportEl?.querySelector(`[data-index="${targetIndex}"]`) as HTMLElement | null;
			if (currentEl) {
					currentEl.classList.remove("highlight-field");
			}
		}, 3500);
	}

	function hopToNextConflict() {
		if (conflictIndices.length === 0) return;
		let nextIndex: number;
		if (activeConflictIndex === null) {
			nextIndex = conflictIndices[0];
		} else {
			const currentIndexInList = conflictIndices.indexOf(activeConflictIndex);
			if (currentIndexInList === -1) {
				const next = conflictIndices.find((idx) => idx > activeConflictIndex!);
				nextIndex = next !== undefined ? next : conflictIndices[0];
			} else {
				const nextPointer = (currentIndexInList + 1) % conflictIndices.length;
				nextIndex = conflictIndices[nextPointer];
			}
		}
		scrollToConflict(nextIndex);
	}

	// function hopToPrevConflict() {
	// 	if (conflictIndices.length === 0) return;
	// 	const currentIdx = getCurrentVisibleIndex();
	// 	const prevList = conflictIndices.filter((idx) => idx < currentIdx);
	// 	const prevIdx = prevList.length > 0 ? prevList[prevList.length - 1] : conflictIndices[conflictIndices.length - 1];
	// 	scrollToConflict(prevIdx);
	// }

	function hopToPrevConflict() {
		if (conflictIndices.length === 0) return;

		let prevIndex: number;
		if (activeConflictIndex === null) {
			prevIndex = conflictIndices[conflictIndices.length - 1];
		} else {
			const currentIndexInList = conflictIndices.indexOf(activeConflictIndex);
			if (currentIndexInList === -1) {
				const prevList = conflictIndices.filter((idx) => idx < activeConflictIndex!);
				prevIndex = prevList.length > 0 ? prevList[prevList.length - 1] : conflictIndices[conflictIndices.length - 1];
			} else {
				const prevPointer = (currentIndexInList - 1 + conflictIndices.length) % conflictIndices.length;
				prevIndex = conflictIndices[prevPointer];
			}
		}
		scrollToConflict(prevIndex);
	}
</script>

<Dialog bind:open onOpenChange={(v) => !v && onclose()}>
	<DialogContent
		class="lg:max-w-[900px] h-[85vh] flex flex-col p-4 gap-0 overflow-hidden
            bg-background/95 dark:bg-background/60 supports-backdrop-filter:bg-background/90 dark:supports-backdrop-filter:bg-background/60 backdrop-blur-xl border-border/50"
		showCloseButton={false}
	>
		<DialogHeader class="mb-2 md:mt-2 md:ml-4 max-md:text-center shrink-0 md:pb-2 relative after:absolute after:bottom-0 after:left-0 after:h-[1px] after:w-full
				after:bg-gradient-to-r after:from-transparent after:via-sidebar-border after:to-transparent
			">
			<div class="inline-flex md:hidden flex-row justify-between">
				<span class="text-muted-foreground text-[10px] font-normal inline-flex flex-col gap-1">
					<HugeiconsIcon
						icon={FileLinkIcon}
						strokeWidth={2}
						class="w-3 h-3 text-muted-foreground"
					/>
					{`{${uris.length}}`}
				</span>
				<DialogTitle class="sm:text-lg ml-2 text-[18px] font-semibold">
					Review download queue...
				</DialogTitle>
				<span class="text-muted-foreground text-[10px] font-normal inline-flex flex-col gap-1">
					<HugeiconsIcon
						icon={GitMergeConflictIcon}
						strokeWidth={2}
						class="w-3 h-3 text-muted-foreground"
					/>
					{`{${conflictCount}}`}
				</span>
			</div>
			<DialogTitle class="sm:text-lg max-md:hidden text-[18px] font-semibold">
				Review download queue...
			</DialogTitle>
			<DialogDescription class="text-sm">
				Verify files & resolve any conflicts before adding in queue
			</DialogDescription>
		</DialogHeader>

		<div
			class="flex-1 overflow-hidden space-y-3 pr-2 min-h-0"
		>
			<!-- Invisible focus anchor to absorb modal focus trap resets -->
			<div tabindex="0" class="sr-only" aria-hidden="true"></div>

			{#if isFetching}
				<div class="flex flex-col items-center justify-center h-full w-full space-y-4 text-muted-foreground min-h-[300px]"
				>
					<HugeiconsIcon icon={LoaderPinwheelIcon} class="w-8 h-8 animate-spin" strokeWidth={2} />
					<p class="text-sm text-center animate-pulse">
						Probing servers and resolving metadata...
					</p>
				</div>
			{:else}
				{#if items.length > 0}
					<Virtual
						bind:this={virtualRef}
						bind:viewportEl={virtualViewportEl}
						rowGap={8}
						overscan={6}
						height={600}
						itemHeight={108}
						items={items}
						getKey={(item, i) => item.uri ?? i}
						overflowX="hidden"
						overflowY="auto"
						autoHeight={true}
						paddingBottom={100}
					>
						{#snippet row(item, index, style)}
							{@const catIcons = getCategoryIcons(item)}
							<div
								{style}
								data-index={index}
								class:highlight-field={highlightedIndex === index}
								class="flex flex-col justify-center p-0.5 md:p-3 border-[0.1px] rounded-3xl transition-colors
								{
									item.uiStatus === 'fatalError'
									? 'border-destructive/40 bg-destructive/5'
									: 'border-[#DDA0DD]/40 bg-[#DDA0DD]/5'
								}"
								>
									<div class="flex items-center md:gap-3 w-full">
										<div class="w-6 h-6 shrink-0 flex items-center justify-center">
											<img
												src={catIcons.light}
												alt="icon"
												class="h-5 w-5 md:w-7 md:h-7 dark:hidden"
											/>
											<img
												src={catIcons.dark}
												alt="icon"
												class="h-5 w-5 md:w-7 md:h-7 hidden dark:block"
											/>
										</div>
										<div class="flex-1 relative">
											<input
												title={item.isMagnet ? "Magnets can't be renamed before downloading" : ""}
												type="text"
												bind:value={item.newName}
												oninput={() => handleNameEdit(index)}
												disabled={
													item.uiStatus === "done" ||
													item.uiStatus === "fatalError" ||
													item.uiStatus === "downloading" ||
													item.isMagnet}
												class="w-[95%] text-[13px] md:text-sm fade-right font-medium bg-transparent border-b
													hover:border-sidebar-border focus:border-muted-foreground focus:outline-none disabled:opacity-70
													disabled:hover:border-transparent transition-colors px-1 truncate"
											/>
										</div>

										<div class="shrink-0 flex items-center justify-end gap-1.5 w-auto pr-1">
											{#if item.uiStatus === "done"}
												<span
													title={item.uiStatus === "done" ?
														"Added to queue" : ""}
												>
													<HugeiconsIcon
														icon={Checkmark}
														class="w-5 h-5 text-muted-foreground"
													/>
												</span>

											{:else if item.uiStatus === "downloading"}
												<div class="bg-primary/10 text-primary p-1.5 rounded-full">
													<HugeiconsIcon icon={LoaderPinwheelIcon} class="w-4 h-4 animate-spin" />
												</div>

											{:else if item.uiStatus === "fatalError"}
												<Button
													title="Remove item"
													variant="ghost"
													size="icon"
													class="h-8 w-8 text-muted-foreground cursor-pointer disabled:cursor-not-allowed hover:text-destructive hover:bg-destructive/10"
													onclick={() => removeItem(index)}
												>
													<HugeiconsIcon
														icon={Delete01Icon}
														class="w-4 h-4"
													/>
												</Button>
											{:else}
												<div class="hidden md:block">
													<Button
														title="Set options"
														variant="ghost"
														size="icon"
														class="h-8 w-8 cursor-pointer disabled:cursor-not-allowed hover:bg-background/60"
														onclick={() => openOptionsDialog(index)}
													>
														<HugeiconsIcon
															icon={DashboardCircleSettingsIcon}
															class="w-4 h-4"
														/>
													</Button>
													<Button
														title={item.uiStatus === "pendingConflict"
															? "Resolve conflict first"
															: "Add in download queue"}
														variant="ghost"
														size="icon"
														class="h-8 w-8 cursor-pointer disabled:opacity-50"
														disabled={item.uiStatus === "pendingConflict" && !item.allowBypassConflict}
														onclick={() => downloadSingle(index)}
													>
														<HugeiconsIcon
															icon={Download03Icon}
															class="w-4 h-4"
														/>
													</Button>
												</div>
											{/if}
										</div>
									</div>

									<div class="flex max-md:mx-2 max-md:mt-2 md:ml-[2.5rem] flex-col space-y-0.5 md:space-y-2">
										{#if item.uiStatus !== "fatalError"}
											<div class="flex items-center keep-in-center gap-1.5 md:mr-2 md:gap-3 text-xs font-medium text-muted-foreground">
												{#if item.uri && item.uri !== null &&  item.uri !== ""}
													<div class="flex items-center">
														<Tooltip.Provider>
															<PopoverTooltip.Root>
																<PopoverTooltip.Trigger>
																	<button
																		title="Click to copy URI"
																		onclick={() => navigator.clipboard.writeText(item.uri!)}
																		class="flex items-center max-md:text-[12px] gap-1.5 cursor-pointer text-muted-foreground"
																	>
																		<HugeiconsIcon
																			icon={FileLinkIcon}
																			strokeWidth={2}
																			class="w-3.5 h-3.5 text-muted-foreground"
																		/>
																		URI
																	</button>
																</PopoverTooltip.Trigger>
																<PopoverTooltip.Content
																	side="top"
																	class="max-w-[300px] z-[200]"
																>
																	<p class="text-xs max-md:text-[12px] break-all">
																		{item.uri}
																	</p>
																</PopoverTooltip.Content>
															</PopoverTooltip.Root>
														</Tooltip.Provider>
													</div>
												{/if}
												<div class="w-[1.5px] h-[16px] rounded-md bg-border"></div>
												<div class="flex items-center gap-1.5 max-md:text-[12px] {item.isResumeSupported ? '' : 'text-destructive/80'}">
													<HugeiconsIcon
														icon={item.isResumeSupported ? Checkmark : Cancel01Icon}
														class="w-3.5 h-3.5"
													/>
													{item.isResumeSupported ? "Resumable" : "Not resumable"}
												</div>
												{#if item.totalLength && item.totalLength !== "0"}
													<div class="w-[1.5px] h-[16px] rounded-md bg-border"></div>
													<div class="flex items-center gap-1.5">
														<HugeiconsIcon
															icon={FloppyDiskIcon}
															class="w-3.5 h-3.5"
														/>
														<span class="max-md:text-[12px]">
															{formatBytes(item.totalLength)}
														</span>
													</div>
												{/if}
												<div class="w-[1.5px] h-[16px] md:hidden rounded-md bg-border"></div>
												<button
													title="Set options"
													class="md:h-8 md:w-8 md:hidden cursor-pointer disabled:cursor-not-allowed hover:bg-background/60"
													onclick={() => openOptionsDialog(index)}
												>
													<HugeiconsIcon
														icon={DashboardCircleSettingsIcon}
														class="w-4 h-4"
													/>
												</button>
												<div class="w-[1.5px] h-[16px] md:hidden rounded-md bg-border"></div>
												<button
													title={item.uiStatus === "pendingConflict"
														? "Resolve conflict first"
														: "Add in download queue"}
													class="md:h-8 md:w-8 md:hidden cursor-pointer disabled:opacity-50"
													disabled={item.uiStatus === "pendingConflict" && !item.allowBypassConflict}
													onclick={() => downloadSingle(index)}
												>
													<HugeiconsIcon
														icon={Download03Icon}
														class="w-4 h-4"
													/>
												</button>
											</div>
										{/if}

										{#if item.probeError && item.uiStatus !== "fatalError"}
											<div class="flex items-center gap-1.5 text-xs text-destructive font-medium">
												<HugeiconsIcon icon={Alert01Icon} class="w-3.5 h-3.5" />
												<span>{item.probeError}</span>
											</div>
										{/if}

										{#if item.uiStatus === "fatalError"}
											<div class="flex items-center gap-1.5 text-xs text-destructive font-medium">
												<HugeiconsIcon icon={Alert01Icon} class="w-3.5 h-3.5" />
												<span>
													{item.probeError || "Invalid or unreachable URL"}
												</span>
											</div>

										{:else if item.uiStatus === "pendingConflict"}
											{@const rules = getConflictButtonsAndRules(item)}

											<div class="flex items-center flex-col md:flex-row gap-3">
												<span class="text-xs text-muted-foreground/90 font-medium flex items-center gap-1">
													<HugeiconsIcon icon={Alert01Icon} class="w-3.5 h-3.5" />
													{item.conflict?.message}
												</span>
												{#if rules.buttons.length > 0}
													<div class="flex flex-wrap items-center gap-2">
														{#each rules.buttons as action (action.action)}
															{#if action.action === "verify" && !item.isMagnet}
																<Button
																	size="sm"
																	variant="outline"
																	class="h-6 text-[11px] px-3 cursor-pointer border-[#DDA0DD]/60"
																	onclick={() => openIntegrityDialog(index)}
																>
																	Verify integrity
																</Button>
															{:else}
																<Button
																	size="sm"
																	variant="outline"
																	class="h-6 text-[11px] px-3 cursor-pointer border-[#DDA0DD]/60"
																	onclick={() => handleConflictAction(index, action.action)}
																>
																	{action.label}
																</Button>
															{/if}
														{/each}
													</div>
												{/if}
											</div>

										{:else if item.isDuplicate && item.allowBypassConflict && item.conflict?.message}
											{@const rules = getConflictButtonsAndRules(item)}

											<div class="flex flex-row items-center gap-3">
												<span class="text-xs text-muted-foreground/90 font-medium flex items-center gap-1">
													<HugeiconsIcon icon={Alert01Icon} class="w-3.5 h-3.5" />
													{item.conflict?.message}
												</span>
												{#if rules.buttons.length > 0}
													<div class="flex flex-wrap items-center gap-2">
														{#each rules.buttons as action (action.action)}
															<Button
																size="sm"
																variant="outline"
																class="h-6 text-xs px-3 cursor-pointer border-[#DDA0DD]/60"
																onclick={() => handleConflictAction(index, action.action)}
															>
																{action.label}
															</Button>
														{/each}
													</div>
												{/if}
											</div>

										{:else if item.selectedAction && item.uiStatus === "ready"}
											<div class="flex items-center font-medium gap-2 text-xs">
												<div class="text-muted-foreground flex gap-1.5 flex-row">
													<HugeiconsIcon icon={Checkmark} class="w-3.5 h-3.5" />
													<span>{getSelectedActionLabel(item.selectedAction)}</span>
												</div>
												<button
													class="text-muted-foreground underline italic cursor-pointer hover:text-foreground ml-2 text-xs
														decoration-muted-foreground/50 underline-offset-2 transition-colors focus:outline-none"
													onclick={() => undoConflictResolution(index)}
												>
													Change
												</button>
											</div>
										{/if}
									</div>
								</div>
						{/snippet}
					</Virtual>

					{#if conflictIndices.length > 0}
						<div
							class="absolute bottom-5 right-6 z-20 flex flex-col items-center gap-0.5 p-1 rounded-full bg-background/90
								dark:bg-card/90 backdrop-blur-md border border-border/80 shadow-lg"
						>
							<Button
								variant="ghost"
								size="icon"
								class="h-7 w-7 rounded-full hover:bg-muted/80
								text-foreground cursor-pointer transition-transform
								active:scale-95"
								title="Previous conflict"
								onclick={hopToPrevConflict}
							>
							<HugeiconsIcon icon={ArrowUp01Icon} class="w-3.5 h-3.5" />
							</Button>

							<div class="flex flex-col items-center justify-center px-1 py-0.5 select-none pointer-events-none">
								<span class="text-[10px] font-semibold text-[#DDA0DD] flex items-center gap-0.5">
									<HugeiconsIcon icon={GitMergeConflictIcon} class="w-3 h-3" />
									<span>{conflictIndices.length}</span>
								</span>
							</div>

							<Button
								variant="ghost"
								size="icon"
								class="h-7 w-7 rounded-full hover:bg-muted/80 text-foreground cursor-pointer transition-transform active:scale-95"
								title="Next conflict"
								onclick={hopToNextConflict}
							>
								<HugeiconsIcon icon={ArrowDown01Icon} class="w-3.5 h-3.5" />
							</Button>
					</div>
					{/if}

				{/if}
			{/if}
		</div>

		<DialogFooter
			class="pt-4 w-full md:gap-6 shrink-0 z-10"
		>
			<div class="w-full md:flex flex-row hidden items-center justify-between">
				<div class="flex flex-row">
					<div class="text-muted-foreground text-xs flex flex-row gap-1.5">
						<HugeiconsIcon
							icon={FileLinkIcon}
							strokeWidth={2}
							class="w-3.5 h-3.5 text-muted-foreground"
						/>
						URIs: {uris.length}
					</div>
					<div class="w-px h-5 mx-2 bg-border"></div>
					<div class="text-muted-foreground text-xs flex flex-row gap-1.5">
						<HugeiconsIcon
							icon={GitMergeConflictIcon}
							strokeWidth={2}
							class="w-3.5 h-3.5 text-muted-foreground"
						/>
						Conflicts: {conflictCount}
					</div>
				</div>
				<div class="space-x-4">
					<Button
						variant="outline"
						onclick={onclose}
						disabled={isFetching || isSubmitting}
						class="disabled:cursor-not-allowed cursor-pointer min-w-24 border-border/60 hover:bg-background/80 transition-colors"
					>
						{allDoneOrFatal ? "Close" : "Cancel"}
					</Button>
					<Button
						title={conflictCount > 0
							? "Resolve all conflict first"
							: "Add all in download queue"}
						onclick={downloadAll}
						class="disabled:cursor-not-allowed cursor-pointer min-w-[140px] shadow-sm transition-all"
						disabled={isFetching ||
							isSubmitting ||
							hasUnresolvedConflicts ||
							allDoneOrFatal ||
							items.length === 0}
					>
						{#if isSubmitting}
							<HugeiconsIcon
								icon={LoaderPinwheelIcon}
								class="w-4 h-4 animate-spin mr-1"
							/>
							Adding...
						{:else}
							Done
						{/if}
					</Button>
				</div>
			</div>


			<div class="w-full flex flex-row justify-between md:hidden">
				<Button
					variant="outline"
					onclick={onclose}
					disabled={isFetching || isSubmitting}
					class="disabled:cursor-not-allowed cursor-pointer min-w-24 border-border/60 hover:bg-background/80 transition-colors"
				>
					{allDoneOrFatal ? "Close" : "Cancel"}
				</Button>
				<Button
					onclick={downloadAll}
					class="disabled:cursor-not-allowed cursor-pointer min-w-[140px] shadow-sm transition-all"
					disabled={isFetching ||
						isSubmitting ||
						hasUnresolvedConflicts ||
						allDoneOrFatal ||
						items.length === 0}
				>
					{#if isSubmitting}
						<HugeiconsIcon
							icon={LoaderPinwheelIcon}
							class="w-4 h-4 animate-spin mr-1"
						/>
						Adding...
					{:else}
						Done
					{/if}
				</Button>
			</div>
		</DialogFooter>
	</DialogContent>
</Dialog>

{#if editingItemIndex !== null}
	<OptionsDialog
		bind:open={optionsDialogOpen}
		initialOptions={{
			...items[editingItemIndex].conflictOptions,
			...items[editingItemIndex].options
		}}
		onsave={saveOptions}
		kind={items[editingItemIndex].isMagnet ? "torrent" : "uri"}
		filename={items[editingItemIndex].newName}
		onclose={() => {
			editingItemIndex = null;
			optionsDialogOpen = false;
		}}
	/>
{/if}

<Dialog
	bind:open={integrityDialogOpen}
	onOpenChange={(v) => !v && closeIntegrityDialog()}
>
	<DialogContent
		showCloseButton={false}
		class="sm:max-w-150 gap-0 overflow-hidden bg-background/95 dark:bg-background/60
			supports-backdrop-filter:bg-background/90 dark:supports-backdrop-filter:bg-background/60 backdrop-blur-xl"
	>
		<div class="px-4 py-2">
			<DialogTitle class="flex items-center gap-2 text-lg font-semibold">
				Verify integrity...
			</DialogTitle>
			<DialogDescription class="mt-1.5 text-sm">
				Validate the exists downloaded file against a known checksum hash to ensure it hasn't been
				corrupted.
			</DialogDescription>
		</div>

		<div class="p-6 space-y-5">
			<div class="flex items-center gap-3 p-3 rounded-xl bg-background border border-border/50 shadow-sm">
				<div class="p-2 rounded-lg bg-muted/40 text-primary">
					<HugeiconsIcon icon={File01Icon} class="w-4 h-4" />
				</div>
				<div class="flex flex-col min-w-0">
					<span class="text-[10px] text-muted-foreground font-semibold uppercase"
						>Target File</span
					>
					<span
						class="text-sm font-medium truncate w-full"
						title={integrityDialogIndex !== null ? items[integrityDialogIndex].newName : ""}
					>
						{let name = integrityDialogIndex !== null ? items[integrityDialogIndex].newName : "Unknown"}
						{truncateMiddle(name, 40)}
					</span>
				</div>
			</div>

			<div class="space-y-4 flex flex-row gap-x-1">
				<div class="space-y-1.5">
					<Label class="text-[13px] font-medium text-foreground/90">
						Hash algorithm
					</Label>
					<div class="relative">
						<Select.Root
							type="single"
							bind:value={integrityAlgorithm}
						>
							<Select.Trigger
								class="rounded-2xl w-[120px] px-3 py-3 text-sm cursor-pointer border border-sidebar-border/80 bg-muted/20
									focus:outline-none focus:ring-1 focus:ring-primary/30 bg-sidebar-background/80 text-left flex
									justify-between items-center transition-colors hover:bg-muted/30"
							>
								{integrityAlgorithm}
							</Select.Trigger>
							<Select.Content class="bg-popover/50 shadow-md p-1 min-w-[8rem]">
								<Select.Group>
									{#if hashAlgorithms.length === 0}
										<Select.Item value="" class="px-2 py-1 text-sm text-muted-foreground! hover:text-primary! cursor-pointer hover:bg-accent">
											No algorithms available.
										</Select.Item>
									{:else}
										{#each hashAlgorithms as algo (algo)}
											<Select.Item
												value={algo}
												class="px-2 py-1 text-sm text-muted-foreground! hover:text-primary! cursor-pointer hover:bg-accent">
													{algo}
											</Select.Item>
										{/each}
									{/if}
								</Select.Group>
							</Select.Content>
						</Select.Root>
					</div>
				</div>

				{#if integrityRequiresChecksum}
					<div class="space-y-1.5 w-full">
						<Label class="text-[13px] font-medium text-foreground/90">
							Checksum hash
						</Label>
						<input
							type="text"
							bind:value={integrityHash}
							class="w-full rounded-2xl px-3 py-2 text-sm outline-none disabled:opacity-50 disabled:cursor-not-allowed
								border border-sidebar-border/80 bg-muted/20 focus:ring-1 focus:ring-primary/30"
						/>
					</div>
				{:else}
					<div class="rounded-lg border border-border/60 bg-muted/20 px-4 py-3 text-xs text-muted-foreground">
						BitTorrent integrity verification does not require a checksum value.
					</div>
				{/if}
			</div>
		</div>

		<DialogFooter class="px-4 py-2 gap-4">
			<Button
				variant="outline"
				onclick={closeIntegrityDialog}
				class="cursor-pointer disabled:cursor-not-allowed"
			>
				Cancel
			</Button>
			<Button
				onclick={confirmIntegrityDialog}
				class="cursor-pointer disabled:cursor-not-allowed"
				disabled={integrityRequiresChecksum && (!integrityHash.trim() || !integrityAlgorithm)}
			>
				Confirm verification
			</Button>
		</DialogFooter>
	</DialogContent>
</Dialog>

<style>
	.fade-right {
		-webkit-mask-image: linear-gradient(to right, black 85%, transparent 100%);
		mask-image: linear-gradient(to right, black 85%, transparent 100%);
	}

	@media (max-width: 767px) {
		.keep-in-center {
			justify-content: center !important;
		}
	}
</style>


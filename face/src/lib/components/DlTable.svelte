<script lang="ts">
	import { mode } from "mode-watcher";
	import { onMount, untrack } from "svelte";
	import type { ItemMetaData } from "$lib/aria2/types";
	import { Badge } from "$lib/components/ui/badge";
	import { HugeiconsIcon } from "@hugeicons/svelte";
	import { Button } from "$lib/components/ui/button";
	import { getFriendlyError } from "$lib/aria2/error";
	import { Label } from "$lib/components/ui/label/index.js";
	import { Checkbox } from "$lib/components/ui/checkbox/index.js";
	import * as Dialog from "$lib/components/ui/dialog";
	import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
	import * as Tooltip from "$lib/components/ui/tooltip/index.js";
	import * as PopoverTooltip from '$lib/components/ui/popovertooltip';
	import { formatSpeed, formatBytes } from "$lib/utils";
	import { useSidebar } from "$lib/components/ui/sidebar";
	import { historySort } from "$lib/sort";
	import {
		statusFilters,
		downloadIcons,
		selectedCategory,
		selectedStatus,
		getDownloadIconName,
		type TableStatusFilter,
	} from "$lib/categorize";
	import {
		BadgeInfoIcon,
		ArrowUp02Icon,
		ArrowDown02Icon,
		CloudUploadIcon,
		MoreVerticalCircle01Icon
	} from "@hugeicons/core-free-icons";
	import { aria2, aria2Store, itemCounts } from "$lib/aria2/client.svelte";
	import {
		selectedGids,
			selectedTotalBytes,
			isActivelySelecting,
			clearSelection,
			toggleSelection,
			selectExclusive,
			setSelection
	} from "$lib/selection";
	import Virtual from '$lib/components/Virtual.svelte';

	/// Some state varibables
  const PAGE_LIMIT = 25;
	const ITEM_STRIDE = 75; // 75px per row

	let currentPage = $state(1);
	let totalPages = $state(1);
	let loading = $state(false);
	let hasMore = $state(true);
  let mounted = false;
	let isDarkMode = $state(false);
	let isSidebarOpen = $state(false);
  let currentFetchId = 0;

	// Fade effect
	let statusScrollContainer: HTMLDivElement | null = $state(null);
	let statusShowLeftFade = $state(false);
	let statusShowRightFade = $state(false);

	// Deletion state
	let deleteDialogOpen = $state(false);
	let deleteWithFiles = $state(false);
	let gidsToDelete: string[] = $state([]);

	// Desktop Drag & Select
	const DRAG_THRESHOLD = 4;
	let dragArmed = $state(false);
	let dragSelecting = $state(false);
	let dragStart = $state({ x: 0, y: 0 });
	let dragCurrent = $state({ x: 0, y: 0 });
	let dragStartContentY = 0;
	let dragWithModifier = false;
	let initialDragSelection: Array<{ gid: string; totalLength: string | null }> = [];
	let lastDragSelectionKey = "";
	let suppressNextClick = false;

	// Mobile Long-Press & Drag Select
	let mobileLongPressTimer: ReturnType<typeof setTimeout> | null = null;
	let isMobileDragSelecting = $state(false);
	let mobileAnchorIndex = -1;
	let touchStartPos = { x: 0, y: 0 };

	// Auto-scroll during drag
	const AUTO_SCROLL_MARGIN = 64;
	const AUTO_SCROLL_MAX_SPEED = 18;
	let autoScrollFrame: number | null = null;
	let virtualViewport = $state<HTMLDivElement | null>(null);
	let tableContainer = $state<HTMLDivElement | null>(null);

	let visibleItems = $derived(aria2Store.items);

	/// Selection by clicks and ctrlkey
	function handleRowClick(event: MouseEvent, item: ItemMetaData) {
		if (suppressNextClick) {
			suppressNextClick = false;
			return;
		}
		if ((event.target as HTMLElement).closest("button, a, input")) return;
		if (event.ctrlKey || event.metaKey || $selectedGids.length > 0) {
			toggleSelection(item.gid, item.totalLength);
		} else {
			selectExclusive(item.gid, item.totalLength);
		}
	}

	/// Row styles
	function getPercentage(completed: string | undefined, total: string | undefined): string {
		const completedBytes = parseInt(completed || "0");
		const totalBytes = parseInt(total || "0");

		if (isNaN(completedBytes) || isNaN(totalBytes) || totalBytes === 0) {
			return "0%";
		}

		const percentage = (completedBytes / totalBytes) * 100;
		return percentage.toFixed(2) + "%";
	}

	function getProgressPercent(item: ItemMetaData): number {
		const completedBytes = parseInt(item.completedLength || "0");
		const totalBytes = parseInt(item.totalLength || "0");

		if (isNaN(completedBytes) || isNaN(totalBytes) || totalBytes === 0) {
			return item.status === "complete" ? 100 : 0;
		}

		return Math.max(0, Math.min(100, (completedBytes / totalBytes) * 100));
	}

	function getRowStyle(item: ItemMetaData): string {
		const progress = item.status === "complete" ? 100 : getProgressPercent(item);
		return `
      background-image: linear-gradient(90deg, rgba(179, 137, 163, 0.3) 0%,
				rgba(179, 137, 163, 0.3) ${progress}%, transparent ${progress}%, transparent 100%);
      clip-path: inset(0 round 0.0rem);
    `;
	}

	/// Status label animation
	function getStatusClass(status: string) {
		switch (status) {
			case "active":
				return "animate-pulse";
			case "waiting":
				return "animate-pulse";
			default:
				return "";
		}
	}

	/// Time formatting
	function formatDate(dateString: string) {
		const hasTimezone = /(?:Z|[+-]\d{2}:?\d{2})$/i.test(dateString);
		const date = new Date(hasTimezone ? dateString : `${dateString}Z`);

		if (Number.isNaN(date.getTime())) return dateString;

		const diffMs = Date.now() - date.getTime();
		if (diffMs < 0) {
			const userLocales = typeof navigator !== "undefined" ? navigator.languages : ["en"];
			const parts = new Intl.DateTimeFormat(userLocales, {
				hour: "numeric",
				minute: "2-digit",
				hour12: true,
				day: "numeric",
				month: "short",
				year: "numeric"
			}).formatToParts(date);

			const p = parts.reduce(
				(acc, part) => {
					acc[part.type] = part.value;
					return acc;
				},
				{} as Record<string, string>
			);

			return `${p.hour}:${p.minute} ${p.dayPeriod} ${p.day} ${p.month} ${p.year}`;
		}

		const diffMin = Math.floor(diffMs / 60000);
		const diffHour = Math.floor(diffMs / 3600000);
		const diffDay = Math.floor(diffMs / 86400000);

		if (diffMin < 1) return "just now";
		if (diffMin < 60) return `${diffMin} min ago`;
		if (diffHour < 24) return `${diffHour} hour${diffHour === 1 ? "" : "s"} ago`;
		if (diffDay < 7) return `${diffDay} day${diffDay === 1 ? "" : "s"} ago`;
		if (diffDay === 7) return "last week";

		const userLocales = typeof navigator !== "undefined" ? navigator.languages : ["en"];
		const parts = new Intl.DateTimeFormat(userLocales, {
			hour: "numeric",
			minute: "2-digit",
			hour12: true,
			day: "numeric",
			month: "short",
			year: "numeric"
		}).formatToParts(date);

		const p = parts.reduce(
			(acc, part) => {
				acc[part.type] = part.value;
				return acc;
			},
			{} as Record<string, string>
		);

		return `${p.hour}:${p.minute} ${p.dayPeriod} ${p.day} ${p.month} ${p.year}`;
	}

	/// Get categorized icons
	function getIcon(item: ItemMetaData) {
		const name = getDownloadIconName(item);
		return isDarkMode ? downloadIcons[name].dark : downloadIcons[name].light;
	}

	/// Select status
	function selectStatus(status: TableStatusFilter) {
		selectedStatus.set(status);
		clearSelection();
	}

	/// Server actions, deletion
	function promptDelete(gids: string[]) {
		if (gids.length === 0) return;
		gidsToDelete = gids;
		deleteWithFiles = false;
		deleteDialogOpen = true;
	}

	function confirmDeletion() {
		aria2.delete(gidsToDelete, deleteWithFiles);
		deleteWithFiles = false;
		deleteDialogOpen = false;
		if (gidsToDelete === $selectedGids) {
			clearSelection();
		}
	}

	async function handleStop(item: ItemMetaData) {
		aria2.stop([item.gid]);
	}

	async function handleRetry(item: ItemMetaData) {
		// aria2.retry(item.gid)
	}

	async function handleResume(item: ItemMetaData) {
		await aria2.resume([item.gid]);
	}

	async function handlePause(item: ItemMetaData) {
		await aria2.pause([item.gid]);
	}

	/// Drag selection
	async function selectAllDbItems() {
		if (visibleItems.length === 0) return;
		// Fetch all matching items from DB for current status & category
		const allItems = await aria2.getAllHistory($selectedStatus, $selectedCategory);
		if (allItems.length > 0) {
			setSelection(allItems.map((i) => ({ gid: i.gid, totalLength: i.totalLength })));
		} else {
			setSelection(visibleItems.map((i) => ({ gid: i.gid, totalLength: i.totalLength })));
		}
	}

	function isEditableTarget(target: EventTarget | null): boolean {
		if (!(target instanceof HTMLElement)) return false;
		return Boolean(target.closest('input, textarea, select, [contenteditable="true"]'));
	}

	function handleKeyDown(event: KeyboardEvent) {
		const key = event.key.toLowerCase();
		if ((event.ctrlKey || event.metaKey) && key === "a") {
			if (isEditableTarget(event.target)) return;
			event.preventDefault();
			void selectAllDbItems();
		}
	}

	function getStatusCount(status: TableStatusFilter): number {
		const counts = $itemCounts;
		if (!counts) return 0;
		if (status === "all") return counts.byCategory?.everything ?? 0;
		return counts.byStatus?.[status] ?? 0;
	}

	/// Desktop: drag & select
	function updateDragSelection() {
		if (!dragSelecting || !virtualViewport || visibleItems.length === 0) return;

		const currentViewportRect = virtualViewport.getBoundingClientRect();
		const currentContentY = dragCurrent.y - currentViewportRect.top + virtualViewport.scrollTop;

		const minY = Math.max(0, Math.min(dragStartContentY, currentContentY));
		const maxY = Math.max(dragStartContentY, currentContentY);

		const fromIndex = Math.max(0, Math.floor(minY / ITEM_STRIDE));
		const toIndex = Math.min(visibleItems.length - 1, Math.floor(maxY / ITEM_STRIDE));

		if (fromIndex > toIndex) return;

		const rangeKey = `${fromIndex}:${toIndex}:${dragWithModifier}`;
		if (rangeKey === lastDragSelectionKey) return;
		lastDragSelectionKey = rangeKey;

		const rangeItems = visibleItems.slice(fromIndex, toIndex + 1);

		if (dragWithModifier) {
			const combined = [...initialDragSelection];
			const seen = new Set(combined.map((i) => i.gid));
			for (const item of rangeItems) {
				if (!seen.has(item.gid)) {
					combined.push({ gid: item.gid, totalLength: item.totalLength });
				}
			}
			setSelection(combined);
		} else {
			setSelection(rangeItems.map((i) => ({ gid: i.gid, totalLength: i.totalLength })));
		}
	}

	function handleDragMove(event: MouseEvent) {
		if (!dragArmed) return;

		const dx = Math.abs(event.clientX - dragStart.x);
		const dy = Math.abs(event.clientY - dragStart.y);

		if (!dragSelecting) {
			if (dx < DRAG_THRESHOLD && dy < DRAG_THRESHOLD) return;
			dragSelecting = true;
			isActivelySelecting.set(true);
			startAutoScroll();
		}

		dragCurrent = { x: event.clientX, y: event.clientY };
		updateDragSelection();
	}

	function handleDragEnd() {
		if (dragSelecting) {
			suppressNextClick = true;
		}

		dragArmed = false;
		dragSelecting = false;
		isActivelySelecting.set(false); // <--- Signal drag end
		lastDragSelectionKey = "";
		stopAutoScroll();

		window.removeEventListener("mousemove", handleDragMove, true);
		window.removeEventListener("mouseup", handleDragEnd, true);
		window.removeEventListener("blur", handleDragEnd);
	}

	function handleSelectionMouseDown(event: MouseEvent) {
		if (event.button !== 0 || !virtualViewport) return;
		const target = event.target as HTMLElement | null;
		if (!target || target.closest('button, a, input, textarea, select, [role="menuitem"]')) return;
		const viewportRect = virtualViewport.getBoundingClientRect();
		dragArmed = true;
		dragSelecting = false;
		dragStart = { x: event.clientX, y: event.clientY };
		dragCurrent = { x: event.clientX, y: event.clientY };
		dragStartContentY = event.clientY - viewportRect.top + virtualViewport.scrollTop;
		dragWithModifier = event.ctrlKey || event.metaKey || event.shiftKey;

		initialDragSelection = visibleItems
			.filter((i) => $selectedGids.includes(i.gid))
			.map((i) => ({ gid: i.gid, totalLength: i.totalLength }));

		lastDragSelectionKey = "";

		window.addEventListener("mousemove", handleDragMove, true);
		window.addEventListener("mouseup", handleDragEnd, true);
		window.addEventListener("blur", handleDragEnd);
	}

	/// Mobile long-press and drag-select
	function handleTouchStart(e: TouchEvent) {
		if (e.touches.length !== 1 || !virtualViewport) return;
		const touch = e.touches[0];
		touchStartPos = { x: touch.clientX, y: touch.clientY };

		const rowEl = (e.target as HTMLElement).closest("[data-row-gid]") as HTMLElement | null;
		if (!rowEl) return;

		const gid = rowEl.dataset.rowGid;
		const itemIndex = visibleItems.findIndex((i) => i.gid === gid);
		if (itemIndex === -1) return;

		if (mobileLongPressTimer) clearTimeout(mobileLongPressTimer);

		mobileLongPressTimer = setTimeout(() => {
			// Trigger haptic feedback if supported
			if (typeof navigator !== "undefined" && navigator.vibrate) {
				navigator.vibrate(40);
			}

			isMobileDragSelecting = true;
			isActivelySelecting.set(true);
			mobileAnchorIndex = itemIndex;
			suppressNextClick = true;
			dragCurrent = { x: touch.clientX, y: touch.clientY };

			const targetItem = visibleItems[itemIndex];
			if (targetItem) {
				toggleSelection(targetItem.gid, targetItem.totalLength);
			}

			// Add non-passive listeners so e.preventDefault() stops the mobile browser from cancelling touch
			window.addEventListener("touchmove", handleActiveTouchMove, { passive: false });
			window.addEventListener("touchend", handleActiveTouchEnd);
			window.addEventListener("touchcancel", handleActiveTouchEnd);

			startAutoScroll();
		}, 400);
	}

	function handleActiveTouchMove(e: TouchEvent) {
		if (!isMobileDragSelecting || e.touches.length !== 1 || !virtualViewport) return;

		e.preventDefault();
		e.stopPropagation();

		const touch = e.touches[0];
		dragCurrent = { x: touch.clientX, y: touch.clientY };
		updateMobileTouchSelection(touch.clientY);
	}

	function updateMobileTouchSelection(clientY: number) {
		if (!virtualViewport || mobileAnchorIndex === -1 || visibleItems.length === 0) return;

		const viewportRect = virtualViewport.getBoundingClientRect();
		const contentY = clientY - viewportRect.top + virtualViewport.scrollTop;
		const currentIndex = Math.max(
			0,
			Math.min(visibleItems.length - 1, Math.floor(contentY / ITEM_STRIDE))
		);

		const fromIndex = Math.min(mobileAnchorIndex, currentIndex);
		const toIndex = Math.max(mobileAnchorIndex, currentIndex);

		const rangeItems = visibleItems.slice(fromIndex, toIndex + 1);
		setSelection(rangeItems.map((i) => ({ gid: i.gid, totalLength: i.totalLength })));
	}

	function handleActiveTouchEnd() {
		if (mobileLongPressTimer) clearTimeout(mobileLongPressTimer);

		window.removeEventListener("touchmove", handleActiveTouchMove);
		window.removeEventListener("touchend", handleActiveTouchEnd);
		window.removeEventListener("touchcancel", handleActiveTouchEnd);

		if (isMobileDragSelecting) {
			isMobileDragSelecting = false;
      isActivelySelecting.set(false);
			mobileAnchorIndex = -1;
			stopAutoScroll();
		}
	}

	function startAutoScroll() {
		if (autoScrollFrame !== null) return;

		const step = () => {
			if (!dragSelecting && !isMobileDragSelecting) {
				stopAutoScroll();
				return;
			}

			let deltaY = 0;
			if (virtualViewport) {
				const rect = virtualViewport.getBoundingClientRect();
				const topEdge = rect.top + AUTO_SCROLL_MARGIN;
				const bottomEdge = rect.bottom - AUTO_SCROLL_MARGIN;

				if (dragCurrent.y < topEdge) {
					const ratio = Math.min(1, (topEdge - dragCurrent.y) / AUTO_SCROLL_MARGIN);
					deltaY = -Math.ceil(ratio * AUTO_SCROLL_MAX_SPEED);
				} else if (dragCurrent.y > bottomEdge) {
					const ratio = Math.min(1, (dragCurrent.y - bottomEdge) / AUTO_SCROLL_MARGIN);
					deltaY = Math.ceil(ratio * AUTO_SCROLL_MAX_SPEED);
				}

				if (deltaY !== 0) {
					virtualViewport.scrollBy({ top: deltaY, left: 0, behavior: "auto" });
					if (isMobileDragSelecting) {
						updateMobileTouchSelection(dragCurrent.y);
					} else if (dragSelecting) {
						updateDragSelection();
					}
				}
			}

			autoScrollFrame = requestAnimationFrame(step);
		};

		autoScrollFrame = requestAnimationFrame(step);
	}

	function stopAutoScroll() {
		if (autoScrollFrame !== null) {
			cancelAnimationFrame(autoScrollFrame);
			autoScrollFrame = null;
		}
	}

	function handleStatusScrollState() {
		if (!statusScrollContainer) return;
		if (window.innerWidth >= 640) {
			statusShowLeftFade = false;
			statusShowRightFade = false;
			return;
		}
		const { scrollLeft, scrollWidth, clientWidth } = statusScrollContainer;
		statusShowLeftFade = scrollLeft > 2;
		statusShowRightFade = scrollLeft + clientWidth < scrollWidth - 2;
	}

	/// A little refresh fn
	async function refresh(page = 1) {
		const fetchId = ++currentFetchId;
		loading = true;

		try {
			const json = await aria2.getHistory({
				page,
				limit: PAGE_LIMIT,
				status: $selectedStatus,
				category: $selectedCategory,
				sortBy: $historySort.sortBy,
				order: $historySort.order
			});

			if (fetchId !== currentFetchId) return;

			if (json?.meta) {
				currentPage = json.meta.currentPage;
				totalPages = json.meta.totalPages;
				hasMore = json.meta.hasMore ?? currentPage < totalPages;
			} else {
				hasMore = false;
			}
		} catch (e) {
			console.error("Refresh error:", e);
		} finally {
			if (fetchId === currentFetchId) {
				loading = false;
			}
		}
		clearSelection();
	}

	/// Load next page when virtual list scrolls near bottom, infinite scroll
	async function loadNextPage() {
		if (loading || !hasMore) return;
		const fetchId = currentFetchId;
		loading = true;

		try {
			const nextPage = currentPage + 1;
			const json = await aria2.getHistory({
				page: nextPage,
				limit: PAGE_LIMIT,
				status: $selectedStatus,
				category: $selectedCategory,
				sortBy: $historySort.sortBy,
				order: $historySort.order
			});

			if (fetchId !== currentFetchId) return;

			if (json?.meta) {
				currentPage = json.meta.currentPage;
				totalPages = json.meta.totalPages;
				hasMore = json.meta.hasMore ?? currentPage < totalPages;
			} else {
				hasMore = false;
			}
		} catch (e) {
			console.error("Error loading next page:", e);
		} finally {
			if (fetchId === currentFetchId) {
				loading = false;
			}
		}
	}

	$effect(() => {
			const sidebar = useSidebar();
		const savedMode = localStorage.getItem("mode-watcher-mode");
		isDarkMode = mode.current === "dark" || savedMode === "dark";
		if (sidebar.open === true) {
			isSidebarOpen = true;
		} else {
			isSidebarOpen = false;
		}
		handleStatusScrollState();
	});

	$effect(() => {
		const _status = $selectedStatus;
		const _cat = $selectedCategory;
		const _sort = $historySort;
		if (!mounted) return;

		untrack(() => {
			currentPage = 1;
			hasMore = true;
			if (virtualViewport) virtualViewport.scrollTop = 0;
			void refresh(1);
		});
	});

	onMount(() => {
		mounted = true;
		aria2.connectDlWs();
		aria2.connectGlobalStatWs();
		window.addEventListener("keydown", handleKeyDown, true);

		void refresh(1);

		return () => {
			window.removeEventListener("keydown", handleKeyDown, true);
			stopAutoScroll();
			handleActiveTouchEnd();
			handleDragEnd();
		};
	});
</script>

<div class="md:space-y-4 mt-4 relative">
	<div
		class="h-12 border-t bg-card/95 backdrop-blur z-1 supports-backdrop-filter:bg-card/60
			border-l w-[-moz-available] w-[-webkit-fill-available] scrollbar-none border-r top-18 my-0 sticky inline-block overflow-x-scroll
			max-sm:mx-2 mx-6 md:mx-10 rounded-t-2xl"
		>
	<div
			bind:this={statusScrollContainer}
			onscroll={handleStatusScrollState}
			onresize={handleStatusScrollState}
			class="h-full overflow-x-auto scrollbar-none"
			class:fade-left={statusShowLeftFade}
			class:fade-right={statusShowRightFade}
		>
			<div class="h-full max-sm:w-[700px] flex justify-around items-center">
				{#each statusFilters as status, i (status)}
					<button
						onclick={() => selectStatus(status)}
						type="button"
						class={`text-[14px] w-full h-full flex flex-col items-center justify-center font-medium transition-colors cursor-pointer
							${i !== statusFilters.length - 1 ? "border-r" : ""}
							${$selectedStatus === status ? "bg-[#DDA0DD]/30" : ""}
							${i === 0 ? "rounded-tl-[14px]" : ""}
							${i === statusFilters.length - 1 ? "rounded-tr-[14px]" : ""}`}
					>
						{status?.charAt(0).toUpperCase() + status.slice(1)}
						{#if $itemCounts?.byCategory.everything > 15}
							<span class="text-[10px]">{`{${getStatusCount(status)}}`}</span>
						{/if}
					</button>
				{/each}
			</div>
		</div>
	</div>

	<!-- Main table container with desktop drag & Mobile touch-drag -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions  -->
	<div
		role="region"
		aria-label="Downloads Table"
		bind:this={tableContainer}
		onmousedown={handleSelectionMouseDown}
		ontouchstart={handleTouchStart}
		ontouchmove={(e) => {
			if (!isMobileDragSelecting && mobileLongPressTimer) {
				const touch = e.touches[0];
				if (touch) {
					const dx = Math.abs(touch.clientX - touchStartPos.x);
					const dy = Math.abs(touch.clientY - touchStartPos.y);
					if (dx > 10 || dy > 10) {
						clearTimeout(mobileLongPressTimer);
						mobileLongPressTimer = null;
					}
				}
			}
		}}
		ontouchend={() => {
			if (mobileLongPressTimer) clearTimeout(mobileLongPressTimer);
		}}
		ontouchcancel={() => {
			if (mobileLongPressTimer) clearTimeout(mobileLongPressTimer);
		}}
		class="border bg-card select-none max-sm:mx-2 mx-6 md:mx-10"
		class:touch-none={isMobileDragSelecting}
	>
		{#if visibleItems.length > 0}
			<Virtual
				bind:viewportEl={virtualViewport}
				items={visibleItems}
				height={Math.max(650, typeof window !== "undefined" ? window.innerHeight - 260 : 600)}
				itemHeight={ITEM_STRIDE}
				overscan={4}
				rowGap={0}
				overflowY="auto"
				class="w-full"
				getKey={(item) => item.gid}
				row={row}
				onEndReached={loadNextPage}
				rowsStyle="
					display: flex;
					align-items: stretch;
					box-sizing: border-box;
					border-top-style: var(--tw-border-style);
					border-top-width: 0.5px;
				"
			/>
			{#if loading}
					<div class="py-2 text-center text-xs text-muted-foreground
					animate-pulse border-t border-border/40">
						Loading more downloads...
					</div>
			{/if}
		{:else}
			<div class="h-[80svh] w-full flex items-center justify-center text-center text-muted-foreground">
				<span class="text-wrap font-normal max-md:text-[16px]">
					No downloads found {`in category ${$selectedCategory ? $selectedCategory + `${" "} ${"and status"} ${$selectedStatus}${"..."}` : ""}`}
				</span>
			</div>
		{/if}

		{#if dragSelecting}
			<div
				class="pointer-events-none fixed z-50 rounded-sm border border-primary/70 bg-primary/20"
				style={`left: ${Math.min(dragStart.x, dragCurrent.x)}px;
					top: ${Math.min(dragStart.y, dragCurrent.y)}px;
					width: ${Math.abs(dragCurrent.x - dragStart.x)}px;
					height: ${Math.abs(dragCurrent.y - dragStart.y)}px;`}
			></div>
		{/if}
	</div>
</div>

<Dialog.Root bind:open={deleteDialogOpen}>
	<Dialog.Content class="sm:max-w-[500px] sm:min-h-[350px]">
		<Dialog.Header>
			<Dialog.Title>Confirm Delete</Dialog.Title>
			<Dialog.Description>
				Are you sure you want to delete this download?
				<br /> this action cannot be undone.
				<br /><br />
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-row gap-2">
			<div class="w-4 h-6 mt-1 inline">
				<Checkbox class="cursor-pointer" id="delete-with-files" bind:checked={deleteWithFiles} />
			</div>
			<div class="inline">
				<Label for="delete-with-files" class="cursor-pointer">Delete files too!?</Label>
				<br />
				<span class="text-[14px] font-normal text-muted-foreground"
					>be careful, it'll delete their actual content from server!!</span
				>
			</div>
		</div>

		<Dialog.Footer class="mt-2">
			<Button class="cursor-pointer" variant="outline" onclick={() => (deleteDialogOpen = false)}>
				Cancel
			</Button>
			<Button class="min-w-24 cursor-pointer" variant="destructive" onclick={confirmDeletion}>
				Delete
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>


<style>
	.fade-right {
		-webkit-mask-image: linear-gradient(to right, black 85%, transparent 100%);
		mask-image: linear-gradient(to right, black 85%, transparent 100%);
	}

	.fade-left {
		-webkit-mask-image: linear-gradient(to left, black 85%, transparent 100%);
		mask-image: linear-gradient(to left, black 85%, transparent 100%);
	}

	.fade-left.fade-right {
		-webkit-mask-image: linear-gradient(to right, transparent 0%, black 15%, black 85%, transparent 100%);
		mask-image: linear-gradient(to right, transparent 0%, black 15%, black 85%, transparent 100%);
	}

	@media (max-width: 767px) {
		.keep-font-small {
			font-size: 14px !important;
		}
	}
</style>

{#snippet row(item: ItemMetaData, index: number, rowStyle: string)}
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
	<div
		data-row-gid={item.gid}
		onclick={(e) => handleRowClick(e, item)}
		data-selected={$selectedGids.includes(item.gid)}
		style={`${rowStyle}; ${getRowStyle(item)}`}
		class="cursor-pointer h-[75px] max-h-fit bg-blend-darken transition-colors hover:bg-muted/50 data-[selected=true]:bg-[#DDA0DD]/40 border-xl"
	>
		<div class="xs:w-12 xs:h-16 h-[inherit] w-8 flex justify-center items-center shrink-0">
			<img
				src={getIcon(item)}
				alt="svg-icon"
				aria-hidden="true"
				class="w-5 sm:w-7 xs:w-6 shrink-0"
			/>
		</div>

	<div class="flex min-w-0 flex-1 items-center">
		<div class="lg:min-w-[70%]">
			<div class="flex flex-col min-h-10 justify-center w-full">
				<!-- // Name, for mobile and desktop-->
				<!-- // visible till xs -->
				<span
					title={item.name || item.gid}
					class="font-medium block xs:hidden text-[13px] float-left max-w-[100%]"
				>
					{(item.name?.length ?? 0) > 15 ? `${item.name?.slice(0, 25)}...${item.name?.slice(-6)}` : item.name || "<Untitled>"}
				</span>
				<!-- // visible till sm -->
				<span
					title={item.name || item.gid}
					class="font-medium block max-xs:hidden sm:hidden text-[14px] float-left"
				>
					{(item.name?.length ?? 0) > 30 ? `${item.name?.slice(0, 30)}...${item.name?.slice(-4)}` : item.name || "<Untitled>"}
				</span>
				<!-- // visible between upper xs and md -->
				<span
					title={item.name || item.gid}
					class="font-medium block max-sm:hidden md:hidden text-[14px] float-left"
				>
					{(item.name?.length ?? 0) > 40 ? `${item.name?.slice(0, 40)}...${item.name?.slice(-8)}` : item.name || "<Untitled>"}
				</span>
				<!-- // visible on upper md -->
				<span
					title={item.name || item.gid}
					class="font-medium hidden md:block text-[16px] float-left"
				>
					{(item.name?.length ?? 0) > 50 ? `${item.name?.slice(0, 50)}...${item.name?.slice(-8)}` : item.name || "<Untitled>"}
				</span>

				{#if item.status === "error"}
					<div class="flex items-center mt-0.5 text-[12px] text-muted-foreground gap-2">
						<span
							class="flex max-sm:hidden items-center md:font-medium gap-0.5 text-[12px] text-destructive max-w-[300px]"
						>
							<span class="flex items-center">
								{#if item.completedLength && item.completedLength !== "0"}
									{getPercentage(item.completedLength!, item.totalLength!)} &nbsp; | &nbsp;
								{/if}
								{#if item.completedLength && item.completedLength !== "0"}
									&nbsp; | &nbsp;
									{formatBytes(item.completedLength)}
								{/if}
								{#if item.totalLength && item.totalLength !== "0"}
									{formatBytes(item.totalLength)}
								{/if}
								{#if item.uploadLength && item.uploadLength !== "0"}
									&nbsp; | &nbsp;
									<HugeiconsIcon
										icon={CloudUploadIcon}
										size={15}
										color="currentColor"
										strokeWidth={1.5}
										class="mx-0.5"
									/>
									{formatBytes(item.uploadLength)}
								{/if}
							</span>
						</span>

						<!-- For mobile devices -->
						<span
							class="flex sm:hidden items-center md:font-medium gap-0.5 text-[12px] text-destructive max-w-[300px]"
						>
							<!--
								percentage, totalLength,
								error, errorMessage
							-->
							<span class="flex flex-col">
								<span class="block">
									{#if item.completedLength && item.completedLength !== "0"}
										{getPercentage(item.completedLength!, item.totalLength!)} &nbsp; | &nbsp;
									{/if}
									{#if item.totalLength && item.totalLength !== "0"}
										{formatBytes(item.totalLength)}
									{/if}
								</span>
								<span class="flex flex-row">
									<span class="sm:hidden">
										{item.status?.charAt(0).toUpperCase() + item.status.slice(1)}
									</span>
									&nbsp; | &nbsp;
									<Tooltip.Provider>
										<PopoverTooltip.Root>
											<PopoverTooltip.Trigger
												class="flex text-destructive items-center cursor-pointer justify-center gap-1"
											>
												{getFriendlyError(item.errorCode)}
												<HugeiconsIcon
													icon={BadgeInfoIcon}
													size={12}
													color="currentColor"
													strokeWidth={2}
												/>
											</PopoverTooltip.Trigger>
											<PopoverTooltip.Content>
												<p
													class="text-wrap text-destructive keep-font-small sm:w-[64]"
													style="overflow-wrap: break-word;"
												>
													{item.errorMessage?.replaceAll("--", "")}
												</p>
											</PopoverTooltip.Content>
										</PopoverTooltip.Root>
									</Tooltip.Provider>
								</span>
							</span>
						</span>
					</div>
				{:else}
					<div class="flex items-center text-[12px] text-muted-foreground gap-2 mt-1">
						{#if (item.completedLength && item.completedLength !== "0") || item.totalLength !== "0" || item.status !== null}
							<div class="max-sm:hidden flex md:font-medium tracking-wide items-center gap-0.5">
								{#if item.completedLength && item.completedLength !== "0"}
									<span>
										{getPercentage(item.completedLength!, item.totalLength!)}
										&nbsp; | &nbsp;
									</span>
								{/if}
								{#if item.completedLength && item.completedLength !== "0"}
									<span>
										{formatBytes(item.completedLength)}
									</span>
								{/if}
								{#if item.totalLength && item.totalLength !== "0"}
									<span>
										{`${item.completedLength && item.completedLength !== "0" ? "/" : ""} ${formatBytes(item.totalLength)}`}
									</span>
								{/if}
								{#if item.uploadLength && item.uploadLength !== "0"}
									<span class="flex flex-row items-center">
										&nbsp; | &nbsp;
										<HugeiconsIcon
											icon={CloudUploadIcon}
											size={15}
											color="currentColor"
											strokeWidth={1.5}
											class="mx-0.5"
										/>
										{formatBytes(item.uploadLength)}
									</span>
								{/if}
								{#if item.status === "complete" && item.completedAt}
									<span>
										&nbsp; | &nbsp;
										{formatDate(item.completedAt)}
									</span>
								{/if}
								{#if item.status && item.status === "active"}
									{@const eta = aria2.calculateETA(item)}
									{#if eta && eta !== "" && eta !== "0sec"}
										<span>
											&nbsp; | &nbsp;
											ETA: {eta}
										</span>
									{/if}
								{/if}
							</div>

							<!-- For mobile devices -->
							<!-- I should've generated by ai -->
							<div class="sm:hidden flex flex-col font-medium text-[11px] tracking-wide gap-0.5">
								<span class="flex flex-row">
									<!-- percentage, no when complete -->
									{#if item.completedLength && item.completedLength !== "0" && item.status !== "complete"}
										<span>
											{getPercentage(item.completedLength!, item.totalLength!)}
											&nbsp; | &nbsp;
										</span>
									{/if}
									<!-- speeds only when active or seeding -->
									{#if item.status !== "complete" && ['active', 'seeding'].includes(item.status)}
										{#if item.downloadSpeed && item.downloadSpeed !== "0"}
											<span class="flex flex-row">
												<HugeiconsIcon
													icon={ArrowDown02Icon}
													size={14}
													color="currentColor"
													strokeWidth={2}
												/>
												{formatSpeed(item.downloadSpeed)}
												&nbsp; | &nbsp;
											</span>
										{/if}
										{#if item.uploadSpeed && item.uploadSpeed !== "0"}
											|
											<span class="flex flex-row">
												<HugeiconsIcon
													icon={ArrowDown02Icon}
													size={14}
													color="currentColor"
													strokeWidth={2}
												/>
												{formatSpeed(item.uploadSpeed)}
												&nbsp; | &nbsp;
											</span>
										{/if}
									{/if}
									<!-- completed length by total length only when not active or seeding -->
									{#if item.completedLength && item.completedLength !== "0" && !['active', 'seeding'].includes(item.status)}
										<span>
											{formatBytes(item.completedLength)}
										</span>
										{#if item.status !== "complete" && item.totalLength && item.totalLength !== "0" }
											<span>
												{`${item.completedLength && item.completedLength !== "0" ? "/" : ""} ${formatBytes(item.totalLength)}`}
											</span>
										{/if}
									{/if}
									<!-- upload Length -->
									{#if item.uploadLength && item.uploadLength !== "0"}
										<span class="flex flex-row items-center">
											&nbsp; | &nbsp;
											<HugeiconsIcon
												icon={CloudUploadIcon}
												size={15}
												color="currentColor"
												strokeWidth={1.5}
												class="mx-0.5"
											/>
											{formatBytes(item.uploadLength)}
											&nbsp; | &nbsp;
										</span>
									{/if}
									<!-- completed time when completed -->
									{#if item.status === "complete" && item.completedAt !== null}
										<span>
											&nbsp; | &nbsp;
											{formatDate(item.completedAt)}
										</span>
									{/if}
									<!-- status -->
									{#if item.status && item.status !== null}
										{#if item.status && item.status !== null && item.totalLength && item.totalLength !== "0" && item.completedLength && item.completedLength !== "0"}
											&nbsp; | &nbsp;
										{/if}
										<span>
											{item.status?.charAt(0).toUpperCase() + item.status.slice(1)}
										</span>
									{/if}
								</span>
								<!-- in col when multiple values -->
								<span class="flex flex-row">
									{#if item.status !== "complete" && ['active', 'seeding'].includes(item.status) && item.totalLength && item.totalLength !== "0" }
										{@const eta = aria2.calculateETA(item)}
										{#if item.completedLength && item.completedLength !== "0"}
											<span>
												{formatBytes(item.completedLength)}
											</span>
										{/if}
										<span>
											{`${item.completedLength && item.completedLength !== "0" ? "/" : ""} ${formatBytes(item.totalLength)}`}
										</span>
										{#if eta && eta !== "" && eta !== "0sec"}
											<span>
												&nbsp; | &nbsp;
												ETA: {eta}
											</span>
										{/if}
									{/if}
								</span>
							</div>
						{/if}
					</div>
				{/if}
			</div>
		</div>
	</div>

		<div class="sm:min-w-[30%]! max-sm:hidden flex justify-center">
			<div class="gap-2 flex flex-col items-center justify-center">
				<Badge
					variant="outline"
					class={`${getStatusClass(item.status)} rounded-xl w-24 py-[12px] shadow-sm`}
				>
					{item.status?.toUpperCase()}
				</Badge>
				<span
					class="flex flex-row align-baseline font-medium tracking-wide text-[12px] items-center gap-0.5 text-muted-foreground"
				>
					{#if item.downloadSpeed && item.downloadSpeed !== "0"}
						<HugeiconsIcon
							icon={ArrowDown02Icon}
							size={14}
							color="currentColor"
							strokeWidth={2}
						/>
						{formatSpeed(item.downloadSpeed)}
					{/if}
					{#if item.uploadSpeed && item.uploadSpeed !== "0"}
						|
						<HugeiconsIcon
							icon={ArrowUp02Icon}
							size={14}
							color="currentColor"
							strokeWidth={1.5}
						/>
						{formatSpeed(item.uploadSpeed)}
					{/if}
					{#if item.status === "error"}
						<Tooltip.Provider>
							<PopoverTooltip.Root>
								<PopoverTooltip.Trigger
									class="flex text-destructive items-center cursor-pointer justify-center gap-1"
								>
									{getFriendlyError(item.errorCode)}
									<HugeiconsIcon
										icon={BadgeInfoIcon}
										size={12}
										color="currentColor"
										strokeWidth={2}
									/>
								</PopoverTooltip.Trigger>
								<PopoverTooltip.Content class="sm:w-[64]">
									<p
										class="text-wrap text-destructive sm:w-[64]"
										style="overflow-wrap: break-word !important; word-break: break-all !important;"
									>
										{item.errorMessage?.replaceAll("--", "")}
									</p>
								</PopoverTooltip.Content>
							</PopoverTooltip.Root>
						</Tooltip.Provider>
					{/if}
				</span>
			</div>
		</div>

		<div class="text-right pr-2 lg:pr-6 flex justify-center" onclick={(e) => e.stopPropagation()}>
			<DropdownMenu.Root>
				<DropdownMenu.Trigger class="cursor-pointer">
					<HugeiconsIcon
						icon={MoreVerticalCircle01Icon}
						size={20}
						color="currentColor"
						strokeWidth={1.5}
					/>
					<span class="sr-only">Open menu</span>
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end" class="p-3 cursor-pointer text-sm">
					{#if ["active"].includes(item.status)}
						<DropdownMenu.Item
							onclick={() => handlePause(item)}
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Pause
						</DropdownMenu.Item>
					{/if}
					{#if ["error", "stopped", "removed"].includes(item.status)}
						<DropdownMenu.Item
							title="Retry to error"
							onclick={() => handleRetry(item)}
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Retry
						</DropdownMenu.Item>
					{/if}
					{#if ["paused"].includes(item.status)}
						<DropdownMenu.Item
							onclick={() => handleResume(item)}
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Resume
						</DropdownMenu.Item>
					{/if}
					{#if ["active", "waiting"].includes(item.status)}
						<DropdownMenu.Item
							title="Stop from downloading"
							onclick={() => handleStop(item)}
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Stop
						</DropdownMenu.Item>
					{/if}
					{#if ["active", "seeding", "waiting", "paused"].includes(item.status)}
						<DropdownMenu.Item
							title="Change options"
							onclick={() => {}}
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Change options
						</DropdownMenu.Item>
					{/if}
					{#if ["error", "stopped"].includes(item.status)}
						<DropdownMenu.Item
							title="Set options"
							onclick={() => {}}
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Set options
						</DropdownMenu.Item>
					{/if}
					{#if ["complete"].includes(item.status)}
						<DropdownMenu.Item
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Redownload
						</DropdownMenu.Item>
					{/if}
					{#if ["seeding"].includes(item.status)}
						<DropdownMenu.Item
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Stop seeding
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Separator />
					{#if item.status}
						<DropdownMenu.Item
							onclick={() => {}}
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Details
						</DropdownMenu.Item>
					{/if}
					{#if item.kind === "uri" || item.sourceUri?.startsWith("magnet")}
						<DropdownMenu.Item
							onclick={() => navigator.clipboard.writeText(item.sourceUri!)}
							class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
								pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
						>
							Copy uri
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Item
						onclick={() => {
							let filesArr = JSON.parse(item.files!);
							navigator.clipboard.writeText(filesArr[0].replace(/\.$/, ""))
						}}
						class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
							pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
					>
						Copy path
					</DropdownMenu.Item>
					<DropdownMenu.Item
						onclick={() => navigator.clipboard.writeText(item.gid)}
						class="flex hover:rounded-xl items-center px-2 py-1.5 rounded-md
							pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-primary!"
					>
						Copy GID
					</DropdownMenu.Item>
					<DropdownMenu.Separator />
					<DropdownMenu.Item
						class="flex text-destructive hover:rounded-xl items-center px-2 py-1.5 rounded-md
							pointer-events-auto cursor-pointer hover:bg-muted-foreground/10! hover:text-destructive/80"
						onclick={() => promptDelete([item.gid])}
					>
						Delete
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		</div>
	</div>
{/snippet}


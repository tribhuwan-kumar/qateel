<script lang="ts" generics="T">
	import type { Snippet } from "svelte";

	type RowKey = string | number;

	type Props = {
		items?: T[];
		height?: number;
		itemHeight?: number;
		overscan?: number;
		class?: string;
		tag?: string;
		getKey?: (item: T, index: number) => RowKey;
		row: Snippet<[T, number, string]>;
		rowGap?: number;
		overflowX?: string;
		overflowY?: string;
		autoHeight?: boolean;
		paddingBottom?: number;
		rowsStyle?: string;
		viewportEl?:  HTMLDivElement | null;
		onEndReached?: () => void;
		endReachedOffset?: number;
	};

	let {
		items = [],
		height = 500,
		itemHeight = 80,
		overscan = 4,
		class: className = "",
		tag = "div",
		getKey,
		row,
		rowGap = 4,
		overflowX = "",
		overflowY = "",
		rowsStyle = "",
		paddingBottom,
		autoHeight = false,
		viewportEl = $bindable(null),
		onEndReached,
		endReachedOffset = 300
	}: Props = $props();

	let scrollTop = $state(0);

	const mappedItems = $derived.by(() => {
		return items.map((item, index) => ({
			item,
			index,
			key: getKey ? getKey(item, index) : index
		}));
	});

	const stride = $derived(itemHeight + rowGap);
	const totalHeight = $derived(autoHeight ? 0 : items.length * stride);
	const startIndex = $derived(autoHeight ? 0 : Math.max(0, Math.floor(scrollTop / stride) - overscan));
	const visibleCount = $derived(autoHeight ? items.length : Math.ceil(height / stride) + overscan * 2);
	const endIndex = $derived(autoHeight ? items.length : Math.min(items.length, startIndex + visibleCount));

	const visibleItems = $derived.by(() => {
		const out: Array<{ item: T; index: number; key: RowKey }> = [];
		for (let i = startIndex; i < endIndex; i++) {
			const item = items[i];
			if (item === undefined) continue;

			out.push({
				item,
				index: i,
				key: getKey ? getKey(item, i) : i
			});
		}
		return out;
	});

	function handleScroll() {
		if (!viewportEl) return;
		scrollTop = viewportEl.scrollTop;

		if (onEndReached) {
			const remaining = viewportEl.scrollHeight - (viewportEl.scrollTop +
					viewportEl.clientHeight);
			if (remaining <= endReachedOffset) {
				onEndReached();
			}
		}
	}

	function rowStyle(index: number) {
		if (autoHeight) {
			return `
				padding: 4px;
				min-height: ${itemHeight}px;
				margin-bottom: ${index === items.length - 1 ? 0 : rowGap}px;
			`;
		}

		return `
			position: absolute;
			left: 0;
			right: 0;
			top: ${index * stride}px;
			height: ${itemHeight}px;
			padding: 4px;
			${rowsStyle}
		`;
	}

	export function scrollToIndex(index: number, behavior: ScrollBehavior = "smooth") {
		if (!viewportEl || index < 0 || index >= items.length) return;

		if (autoHeight) {
			const targetEl = viewportEl.querySelector(`[data-index="${index}"]`) as HTMLElement | null;
			if (targetEl) {
				const targetTop = targetEl.offsetTop - (viewportEl.clientHeight - targetEl.offsetHeight) / 2;
				viewportEl.scrollTo({ top: Math.max(0, targetTop), behavior });
				return;
			}
		}

		const targetTop = index * stride - (height - itemHeight) / 2;
		viewportEl.scrollTo({ top: Math.max(0, targetTop), behavior });
	}
</script>

<svelte:element
	this={tag}
	bind:this={viewportEl}
	data-virtual-viewport="true"
	onscroll={handleScroll}
	class={className}
	style={`
		height:${height}px;
		overflow-x:${overflowX};
		overflow-y:${overflowY};
		position:relative;
		padding-bottom:${paddingBottom}px;
	`}
>
	{#if autoHeight}
		<div
			style={`
				min-height:${height}px;
				position:relative;
			`}
		>
			{#each mappedItems as r (r.key)}
				{@render row(r.item, r.index, rowStyle(r.index))}
			{/each}
		</div>
	{:else}
		<div
			style={`
				height:${totalHeight}px;
				position:relative;
			`}
		>
			{#each visibleItems as r (r.key)}
				{@render row(r.item, r.index, rowStyle(r.index))}
			{/each}
		</div>
	{/if}
</svelte:element>


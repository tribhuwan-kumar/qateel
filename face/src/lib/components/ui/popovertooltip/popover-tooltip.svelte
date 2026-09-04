<script lang="ts">
	import { MediaQuery } from 'svelte/reactivity';
	import type { PopoverRootProps, TooltipRootProps } from 'bits-ui';
	import { Popover } from '$lib/components/ui/popover';
	import { Tooltip } from '$lib/components/ui/tooltip';

	let { ...restProps }: PopoverRootProps & TooltipRootProps = $props();

	const isTouch = new MediaQuery('any-hover: none', false);
</script>

{#if isTouch.current}
	<Popover {...restProps}>
		{@render restProps.children?.()}
	</Popover>
{:else}
	<Tooltip delayDuration={0} {...restProps}>
		{@render restProps.children?.()}
	</Tooltip>
{/if}

<script lang="ts">
	import { MediaQuery } from 'svelte/reactivity';
	import type { ClassArray, ClassDictionary } from 'clsx';
	import { Popover as PopoverPrimitive } from 'bits-ui';
	import { Tooltip as TooltipPrimitive } from 'bits-ui';
	import { cn } from '$lib/utils';
	import * as Popover from '$lib/components/ui/popover';
	import * as Tooltip from '$lib/components/ui/tooltip';

	type Props = {
		class?: string | ClassArray | ClassDictionary | null | undefined;
	};

	let {
		class: className,
		...restProps
	}: Props & PopoverPrimitive.ContentProps & TooltipPrimitive.ContentProps = $props();

	const isTouch = new MediaQuery('any-hover: none', false);
</script>

{#if isTouch.current}
	<Popover.Content class={cn('px-3 py-1.5 text-sm', className)} side="top" {...restProps}>
		{@render restProps.children?.()}
	</Popover.Content>
{:else}
	<Tooltip.Content class={cn('font-medium', className)} {...restProps}>
		{@render restProps.children?.()}
	</Tooltip.Content>
{/if}

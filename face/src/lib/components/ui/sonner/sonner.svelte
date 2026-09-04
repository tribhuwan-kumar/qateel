<script lang="ts">
	import { Toaster as Sonner, type ToasterProps as SonnerProps } from "svelte-sonner";
	import { onMount } from "svelte";
	import { mode } from "mode-watcher";
	import { HugeiconsIcon } from "@hugeicons/svelte"
	import { Loading03Icon } from '@hugeicons/core-free-icons';
	import { CheckmarkCircle02Icon } from '@hugeicons/core-free-icons';
	import { MultiplicationSignCircleIcon } from '@hugeicons/core-free-icons';
	import { InformationCircleIcon } from '@hugeicons/core-free-icons';
	import { Alert02Icon } from '@hugeicons/core-free-icons';

	let { ...restProps }: SonnerProps = $props();
	let position = $state<"top-center" | "bottom-right">("bottom-right");

	onMount(() => {
		const mq = window.matchMedia("(max-width: 768px)");
		const update = () => {
			position = mq.matches ? "top-center" : "bottom-right";
		};

		update();
		mq.addEventListener("change", update);

		return () => mq.removeEventListener("change", update);
	});
</script>

<Sonner
	theme={mode.current}
	class="toaster group z-100!"
	position={position}
	style="--normal-bg: var(--color-popover); --normal-text: var(--color-popover-foreground); --normal-border: var(--color-border);"
  visibleToasts={10}
	{...restProps}
>
	{#snippet loadingIcon()}
		<HugeiconsIcon icon={Loading03Icon} strokeWidth={2} class="size-4 animate-spin" />
	{/snippet}
	{#snippet successIcon()}
		<HugeiconsIcon icon={CheckmarkCircle02Icon} strokeWidth={2} class="size-4" />
	{/snippet}
	{#snippet errorIcon()}
		<HugeiconsIcon icon={MultiplicationSignCircleIcon} strokeWidth={2} class="size-4" />
	{/snippet}
	{#snippet infoIcon()}
		<HugeiconsIcon icon={InformationCircleIcon} strokeWidth={2} class="size-4" />
	{/snippet}
	{#snippet warningIcon()}
		<HugeiconsIcon icon={Alert02Icon} strokeWidth={2} class="size-4" />
	{/snippet}
</Sonner>

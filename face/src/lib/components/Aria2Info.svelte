<script lang="ts">
  import * as Dialog from '$lib/components/ui/dialog';
  import { HugeiconsIcon } from '@hugeicons/svelte';
  import { CodeIcon } from '@hugeicons/core-free-icons';
  import { aria2 } from '$lib/aria2/client.svelte';
  import { JsonView } from '@zerodevx/svelte-json-view'

  let {
    open = $bindable(false)
  } = $props<{
    open: boolean;
  }>();

  let loading = $state(false);
  let error = $state<string | null>(null);
  let info = $state<Record<string, unknown>>({});

  export function openDialog() {
    open = true;
    void fetchInfo();
  }

  async function fetchInfo() {
    loading = true;
    error = null;
    try {
      const result = await aria2.getAria2Info();
      info = (result ?? {}) as Record<string, unknown>;
    } catch (err) {
      error = err instanceof Error ? err.message : 'Failed to fetch aria2 info';
    } finally {
      loading = false;
    }
  }

  $effect(() => {
    if (!open) return;
    void fetchInfo();
  });

</script>

<Dialog.Root
	bind:open
	onOpenChange={(v) => (open = v)}
>
  <Dialog.Content
		showCloseButton={false}
		class="sm:max-w-4xl w-[95vw] h-[85vh] gap-1 py-5 px-8
			bg-background/90 dark:bg-background/60 supports-backdrop-filter:bg-background/90 dark:supports-backdrop-filter:bg-background/60 backdrop-blur-xl"
	>
    <Dialog.Header class="flex h-fit flex-row items-start justify-between gap-4 shrink-0 pb-2 relative after:absolute after:bottom-0 after:left-0 after:h-[1px] after:w-full
				after:bg-gradient-to-r after:from-transparent after:via-sidebar-border after:to-transparent">
      <div>
        <Dialog.Title class="font-semibold text-xl">
					Aria2 info...
				</Dialog.Title>
        <Dialog.Description class="mt-1">
          Version, libraries and system details
        </Dialog.Description>
      </div>
      <div
        title="JSON view"
        class="flex h-8 w-8 cursor-pointer items-center justify-center rounded-full transition-colors"
      >
        <HugeiconsIcon icon={CodeIcon} class="h-5 w-5" strokeWidth={2} />
      </div>
    </Dialog.Header>

    {#if loading}
      <div class="flex h-[65vh] md:h-[70vh] items-center justify-center text-sm text-muted-foreground">
        Loading aria2 info…
      </div>
    {:else if error}
      <div class="h-[65vh] md:h-[70vh] rounded-lg border border-destructive/30 bg-destructive/10 p-4 text-sm text-destructive">
        {error}
      </div>
    {:else}
			<div class="json-class text-xs sm:text-sm h-[65vh] md:h-[70vh] overflow-x-scroll overflow-y-scroll z-100">
				<JsonView
					json={info}
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


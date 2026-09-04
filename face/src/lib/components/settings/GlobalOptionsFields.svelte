<script lang="ts">
  import Label from "$lib/components/ui/label/label.svelte";
  import * as Tooltip from "$lib/components/ui/tooltip";
	import * as PopoverTooltip from '$lib/components/ui/popovertooltip';
  import * as Select from "$lib/components/ui/select";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { HelpCircleIcon } from "@hugeicons/core-free-icons";
  import type { AnyField } from "$lib/components/settings/global-options-schema";

  let {
    field,
		cmdDraft = $bindable(),
    globalDraft = $bindable(),
  }: {
    field: AnyField,
    cmdDraft: any,
		globalDraft: any,
  } = $props();

  const isReadonly = $derived(field.target === "readonly");
  const draft = $derived(field.target === "cmd" ? cmdDraft : globalDraft);

  const getValue = () => draft[field.key] ?? "";

  const setValue = (val: any) => {
    if (isReadonly) return;
    if (field.target === "cmd") cmdDraft[field.key] = val;
    else globalDraft[field.key] = val;
  };

  const getBool = () => getValue() === "true";
  const getSlider = () => {
    if (field.type !== "slider") return 0;
    const raw = getValue();
    return field.decode ? field.decode(raw) : (Number(raw) || field.min);
  };
</script>

<div
	id={`field-${field.key}`}
	class={`space-y-2 rounded-2xl border border-sidebar-border dark:border-sidebar-border/60
		p-3 transition-colors duration-500 shadow-xs ${isReadonly ? "cursor-not-allowed" : ""}`}
>
  <div class="flex items-center justify-between gap-3">
    <div class="flex items-center gap-1">
      <Label
				class="block text-xs font-medium tracking-wide text-sidebar-foreground/70">
					{field.label}
			</Label>
			<Tooltip.Provider>
				<PopoverTooltip.Root>
					<PopoverTooltip.Trigger>
						<button
							type="button"
							class="inline-flex cursor-pointer items-center text-muted-foreground hover:text-foreground"
						>
							<HugeiconsIcon
								icon={HelpCircleIcon}
								strokeWidth={2} class="size-3.5"
							/>
						</button>
					</PopoverTooltip.Trigger>
					<PopoverTooltip.Content side="top" class="max-w-[300px] z-[200]">
						<p class="text-xs">
							{field.description}
						</p>
					</PopoverTooltip.Content>
				</PopoverTooltip.Root>
			</Tooltip.Provider>
    </div>

    {#if isReadonly}
      <span class="text-[10px] uppercase tracking-wider text-muted-foreground/60 font-semibold border rounded px-1.5 py-0.5">
				Read only
			</span>
    {/if}

    {#if field.type === "slider"}
      <span class="text-xs text-muted-foreground">
        {field.format ? field.format(getSlider()) : getSlider()}
        {#if !field.format && field.unit} {field.unit}{/if}
      </span>
    {/if}
  </div>

  {#if field.type === "text"}
    <input
			type="text"
			value={getValue()}
			placeholder={field.placeholder}
			disabled={isReadonly}
			oninput={(e) => setValue(e.currentTarget.value)}
			class="w-full rounded-2xl px-3 py-2 text-sm outline-none disabled:opacity-50 disabled:cursor-not-allowed
				border border-sidebar-border/80 bg-muted/20 focus:ring-1 focus:ring-primary/30"
		/>
  {:else if field.type === "textarea"}
    <textarea
			rows={field.rows ?? 4}
			value={getValue()}
			placeholder={field.placeholder}
			disabled={isReadonly}
			oninput={(e) => setValue(e.currentTarget.value)}
			class="w-full rounded-2xl px-3 py-2 text-sm outline-none disabled:opacity-50 disabled:cursor-not-allowed
				border border-sidebar-border/80 bg-muted/20 focus:ring-1 focus:ring-primary/30"
		></textarea>
  {:else if field.type === "select"}
    <Select.Root
			type="single"
			value={getValue()}
			disabled={isReadonly}
			onValueChange={setValue}
		>
      <Select.Trigger
				class="w-full rounded-md cursor-pointer border border-sidebar-border/80 bg-muted/20 px-3 py-2 text-left text-sm outline-none ring-0"
			>
					{getValue() || "Select..."}
			</Select.Trigger>
      <Select.Content class="bg-sidebar-background/95 z-300">
        <Select.Group>
          {#each field.options as option (option)}
            <Select.Item
							class="cursor-pointer text-muted-foreground! hover:text-primary!"
							value={option}
						>
							{option}
						</Select.Item>
          {/each}
        </Select.Group>
      </Select.Content>
    </Select.Root>

  {:else if field.type === "slider"}
    <input
      type="range"
			min={field.min}
			max={field.max}
			step={field.step}
			value={getSlider()}
			disabled={isReadonly}
			oninput={(e) => {
				const val = Number(e.currentTarget.value);
					setValue(field.encode ? field.encode(val) : String(val));
				}}
			class="custom-slider disabled:cursor-not-allowed"
		/>

  {:else if field.type === "boolean"}
    <label class="relative inline-flex cursor-pointer items-center">
      <input
				type="checkbox"
				checked={getBool()}
				disabled={isReadonly}
				onchange={(e) => setValue(e.currentTarget.checked ? "true" : "false")}
				class="peer sr-only"
			/>
      <div class="peer h-8 w-16 rounded-full bg-muted-foreground/10 border-2 border-sidebar-border
				transition-colors peer-checked:bg-primary peer-disabled:opacity-50 peer-disabled:cursor-not-allowed"
			>
			</div>
      <div class="absolute left-[2px] top-[4px] h-6 w-6 peer-checked:translate-x-9 rounded-full bg-background
				shadow transition-transform peer-disabled:cursor-not-allowed"
			>
			</div>
    </label>
  {/if}
</div>

<style>
	:global(.highlight-field) {
		--highlight-border-width: 2px;
		position: relative;
		border: var(--highlight-border-width) solid transparent;
	}

	@supports not (background: paint(something)) {
		:global(.highlight-field::before) {
			background-image: conic-gradient(var(--color-ring) 80%, #DDA0DD 88%, #f1d9f1 92%, var(--color-ring) 100%);
		}
	}

	:global(.highlight-field:hover::before) {
		animation-play-state: paused;
	}

	@property --angle {
		syntax: "<angle>";
		inherits: true;
		initial-value: 0turn;
	}

	@keyframes spin {
		to {
			--angle: 1turn;
		}
	}

	:global(.highlight-field::before) {
		content: " ";
		position: absolute;
		inset: calc(var(--highlight-border-width) * -1);
		z-index: -1;
		border: inherit;
		border-radius: inherit;
		background-image: conic-gradient(from var(--angle), var(--color-ring) 80%, #DDA0DD 88%, #f1d9f1 92%, var(--color-ring) 100%);
		background-origin: border-box;
		-webkit-mask:
			linear-gradient(black, black) content-box,
			linear-gradient(black, black);
		mask: linear-gradient(black, black),
					linear-gradient(black, black);
		-webkit-mask-clip: content-box, border-box;
		mask-clip: content-box, border-box;
		-webkit-mask-composite: xor;
		mask-composite: exclude;
		animation: spin 3s linear infinite;
	}
</style>


<script lang="ts">
  import { Label } from "$lib/components/ui/label";
	import { HugeiconsIcon } from "@hugeicons/svelte";
  import * as Select from "$lib/components/ui/select";
  import * as Tooltip from "$lib/components/ui/tooltip";
  import { HelpCircleIcon } from "@hugeicons/core-free-icons";
  import type { AnyField } from "$lib/components/settings/options-schema";

  let {
    schema,
    value = $bindable()
  }: {
    schema: AnyField,
    value: any,
	} = $props();

  function getBool() {
    return value === "true";
  }

  function handleBoolChange(e: Event) {
    const target = e.currentTarget as HTMLInputElement;
    value = target.checked ? "true" : "false";
  }

  function getSliderValue(): number[] {
    if (schema.type !== "slider") return [0];
    const decode = schema.decode || ((v: any) => Number(v) || schema.min);
    return [decode(value)];
  }

  function handleSliderChange(v: number) {
    if (schema.type !== "slider") return;
    if (v) {
      const encode = schema.encode || ((n: any) => n.toString());
      value = encode(v);
    }
  }

  function getFormattedSliderValue(): string {
    if (schema.type !== "slider") return "";
    const decode = schema.decode || ((v: any) => Number(v) || schema.min);
    const format = schema.format || ((v: any) => (schema.unit ? `${v} ${schema.unit}` : v.toString()));
    return format(decode(value));
  }
</script>

<div
	id={`field-${schema.key}`}
	class={`space-y-2 rounded-2xl border border-sidebar-border dark:border-sidebar-border/60
		p-3 transition-colors duration-500 shadow-xs`}
>
  <div class="flex items-center justify-between">
    <div class="flex items-center gap-1">
			<Label class="text-xs font-medium tracking-wide text-sidebar-foreground/70">
				{schema.label}
			</Label>
			{#if schema.description}
				<Tooltip.Root>
					<Tooltip.Trigger>
						<button
							type="button"
							class="inline-flex cursor-pointer items-center text-muted-foreground hover:text-foreground"
						>
							<HugeiconsIcon
								icon={HelpCircleIcon}
								strokeWidth={2} class="size-3.5"
							/>
						</button>
					</Tooltip.Trigger>
					<Tooltip.Content
						side="top"
						class="max-w-xs z-[200]">
						<p class="text-xs">
							{schema.description}
						</p>
					</Tooltip.Content>
				</Tooltip.Root>
			{/if}
    </div>

    {#if schema.type === "slider"}
      <span class="text-xs text-muted-foreground">
				{getFormattedSliderValue()}
      </span>
    {/if}
  </div>

  {#if schema.type === 'text'}
    <input
			type="text"
			bind:value
      placeholder={schema.placeholder}
			class="w-full rounded-2xl px-3 py-2 text-sm outline-none disabled:opacity-50 disabled:cursor-not-allowed
				border border-sidebar-border/80 bg-muted/20 focus:ring-1 focus:ring-primary/30"
		/>

  {:else if schema.type === 'textarea'}
    <textarea
			bind:value
      rows={schema.rows || 3}
      placeholder={schema.placeholder}
			class="w-full rounded-2xl px-3 py-2 text-sm outline-none disabled:opacity-50 disabled:cursor-not-allowed
				border border-sidebar-border/80 bg-muted/20 focus:ring-1 focus:ring-primary/30"
		></textarea>

  {:else if schema.type === 'boolean'}
    <label class="relative inline-flex cursor-pointer items-center">
      <input
        type="checkbox"
        checked={getBool()}
				class="peer sr-only"
        onchange={handleBoolChange}
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

  {:else if schema.type === 'slider'}
    <div class="flex items-center gap-4 mt-1 mb-1">
			<input
				type="range"
				min={schema.min}
				max={schema.max}
				step={schema.step}
				value={getSliderValue()}
				oninput={(e) => {
					const val = Number(e.currentTarget.value);
					handleSliderChange(val)
				}}
				class="custom-slider disabled:cursor-not-allowed"
			/>
      <!-- tabular-nums keeps the text from jittering as numbers change size -->
      <!-- <span class="min-w-[4rem] text-right text-xs tabular-nums text-muted-foreground "> -->
      <!--   {getFormattedSliderValue()} -->
      <!-- </span> -->
    </div>

  {:else if schema.type === 'select'}
    <Select.Root
      type="single"
      value={value ?? ""}
      onValueChange={(v) => { if (v) value = v; }}
    >
      <Select.Trigger
				class="w-full rounded-md border border-sidebar-border/80 bg-muted/20 px-3 py-2 text-left text-sm outline-none ring-0"
			>
				{value || "Select..."}
      </Select.Trigger>
      <Select.Content class="bg-sidebar-background/95 z-300">
        <Select.Group>
          {#each schema.options as opt (opt)}
            <Select.Item
              value={opt}
							class="cursor-pointer text-muted-foreground! hover:text-primary!"
            >
              {opt}
            </Select.Item>
          {/each}
        </Select.Group>
      </Select.Content>
    </Select.Root>
  {/if}
</div>


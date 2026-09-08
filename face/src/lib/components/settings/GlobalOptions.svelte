<script lang="ts">
	import { mode } from "mode-watcher";
  import { toast } from "svelte-sonner";
	import { quintOut } from "svelte/easing";
	import { truncateText } from "$lib/utils";
	import { fly, fade } from 'svelte/transition';
	import { aria2 } from "$lib/aria2/client.svelte";
	import { HugeiconsIcon } from "@hugeicons/svelte";
	import { Button } from "$lib/components/ui/button";
	import { type Aria2CmdOptions, type Aria2GlobalOptions } from "$lib/aria2/types";
  import { settingsSections } from "$lib/components/settings/global-options-schema";
	import { Search01Icon, SidebarLeftIcon, InformationCircleIcon } from "@hugeicons/core-free-icons";
	import { SettingIcons, type SettingsIconKey } from "$lib/components/settings/global-options-schema";
	import * as Dialog from "$lib/components/ui/dialog";
	import GlobalOptionsFields from "$lib/components/settings/GlobalOptionsFields.svelte";

	import OptionsLight from "../../../assets/svgs/options-light.svg"
	import Options from "../../../assets/svgs/options.svg"

  let { open = $bindable(false),
		onsave
	} : {
		open: boolean,
		onsave?: () => void
	} = $props();

  type DraftState = {
    global: Partial<Aria2GlobalOptions>;
    cmd: Partial<Aria2CmdOptions>;
  };

  let workingDraft = $state<DraftState>({ global: {}, cmd: {} });
  let initialDraft = $state<DraftState>({ global: {}, cmd: {} });

  let isDarkMode = $state(false);
  let isSaving = $state(false);
	let isFetching = $state(false);
  let sidebarOpen = $state(false);
	let previousOpen = $state(false);
	let mobileSearchOpen = $state(false);
  let showUnsavedChangesDialog = $state(false);
	let activeSectionId = $state(settingsSections[0].id);
	let sectionTransitionDelay = $state(0);

  let activeSection = $derived(settingsSections.find((s) => s.id === activeSectionId));

  // Search state
  let searchQuery = $state("");
  let searchContainer: HTMLDivElement | null = $state(null);
  let sidebarContainer: HTMLElement | null = $state(null);

	function fuzzyTest(itemString: string, query: string): boolean {
		const cleanItem = itemString.toLowerCase();
		// "abc" into a regex like /a.*b.*c/
		const regexStr = query.toLowerCase().split('').join('.*');
		const regex = new RegExp(regexStr);
		return regex.test(cleanItem);
	}

	let searchResults = $derived.by(() => {
		if (!searchQuery.trim()) return [];
		const query = searchQuery.trim();
		const results: { sectionId: string, key: string }[] = [];

		settingsSections.forEach(section => {
			section.fields.forEach(field => {
				if (fuzzyTest(field.key, query) || fuzzyTest(field.label, query)) {
					results.push({ sectionId: section.id, key: field.key });
				}
			});
		});
		return results;
	});

  const cloneDraft = <T>(value: T): T => {
    return JSON.parse(JSON.stringify(value ?? {}));
  };

	const hasGlobalChanges = () => JSON.stringify(workingDraft.global) !== JSON.stringify(initialDraft.global);
  const hasCmdChanges = () => JSON.stringify(workingDraft.cmd) !== JSON.stringify(initialDraft.cmd);
	const hasUnsavedChanges = () => hasGlobalChanges() || hasCmdChanges();

  const loadOptions = async () => {
    isFetching = true;
    try {
			// `getGlobalOption` returns both `global` and `cmd` options
      const currentGlobal = await aria2.getGlobalOption();

      const fetchedState: DraftState = {
        global: cloneDraft(currentGlobal),
        cmd: cloneDraft(currentGlobal),
      };

      initialDraft = cloneDraft(fetchedState);
      workingDraft = cloneDraft(fetchedState);
    } catch (e: any) {
      toast.error("Failed to load options:", {
        description: e.message || e,
        richColors: true,
        style: "cursor: pointer;"
      });
    } finally {
      isFetching = false;
    }
  };

  const save = async (): Promise<boolean> => {
    isSaving = true;
    try {
			const globalChanged = hasGlobalChanges();
      const cmdChanged = hasCmdChanges();

			if (!globalChanged && !cmdChanged) {
				return true;
			}

			const validGlobalKeys = new Set(
					settingsSections.flatMap(s => s.fields)
						.filter(f => f.target === "global")
						.map(f => f.key)
			);

			const validCmdKeys = new Set(
        settingsSections.flatMap(s => s.fields)
          .filter(f => f.target === "cmd")
          .map(f => f.key)
      );

			if (globalChanged) {
				// clean and push global options via json-rpc
				const cleanedGlobal: Partial<Aria2GlobalOptions> = {};
				for (const [key, value] of Object.entries(workingDraft.global)) {
					// remove `&& value.trim() !== ""` cause sometime need to save empty value to disable
					if (validGlobalKeys.has(key as any) && typeof value === "string") {
						cleanedGlobal[key as keyof Aria2GlobalOptions] = value as any;
					}
				}
				console.log("Setting global options:", cleanedGlobal);
				if (Object.keys(cleanedGlobal).length > 0) {
					await aria2.changeGlobalOption(cleanedGlobal);
				}
			}

			if (cmdChanged) {
				const cleanedCmd: Partial<Aria2CmdOptions> = {};
				for (const [key, value] of Object.entries(workingDraft.cmd)) {
					// remove `&& value.trim() !== ""` cause sometime need to save empty value to disable
					if (validCmdKeys.has(key as any) && typeof value === "string") {
						cleanedCmd[key as keyof Aria2CmdOptions] = value as any;
					}
				}
				console.log("Setting cmd options:", cleanedCmd);
				if (Object.keys(cleanedCmd).length > 0) {
					await aria2.changeCmdOption(cleanedCmd);
				}
			}

      initialDraft = cloneDraft(workingDraft);
      onsave?.();
      return true;
    } catch (e: any) {
      toast.error(`Failed to save settings:`, {
				description: e,
				richColors: true,
				style: "cursor: pointer;"
			});
      return false;
    } finally {
      isSaving = false;
    }
  };

  const jumpToField = (sectionId: string, fieldKey: string) => {
		sectionTransitionDelay = 150;
    activeSectionId = sectionId;
    searchQuery = "";
		mobileSearchOpen = false;
    sidebarOpen = false;
    setTimeout(() => {
      const el = document.getElementById(`field-${fieldKey}`);
      if (el) {
        el.scrollIntoView({ behavior: "smooth", inline: "center" });
        el.classList.add("highlight-field");
        setTimeout(() => el.classList.remove("highlight-field"), 5000);
      }
    }, 700);
  };

	function resolveIcon(iconKey: SettingsIconKey): string {
		const theme = isDarkMode ? 'dark' : 'light';
		return SettingIcons[iconKey][theme];
	}

  $effect(() => {
		console.log(`Open: ${open} and previousOpen: ${previousOpen}`)
    if (open && !previousOpen) {
			sectionTransitionDelay = 0;
      showUnsavedChangesDialog = false;
      searchQuery = "";
			loadOptions();
    }
    if (previousOpen && !open && hasUnsavedChanges()) {
			showUnsavedChangesDialog = true;
			open = true;
    }
    previousOpen = open;
  });

  $effect(() => {
    function handleClickOutside(event: any) {
			const clickedOutsideSearch = !searchContainer || !searchContainer.contains(event.target);
			const clickedOutsideSidebar = !sidebarContainer || !sidebarContainer.contains(event.target);
			const clickedOnToggle = (event.target as HTMLElement)?.closest('[data-sidebar-toggle]');
			if (clickedOutsideSearch) {
				searchResults = [];
				searchQuery = "";
			}
			if (clickedOutsideSidebar && !clickedOnToggle) {
				sidebarOpen = false;
			}
    }

    document.addEventListener("click", handleClickOutside, true);
    return () => {
      document.removeEventListener("click", handleClickOutside, true);
    };
  });

	$effect(() => {
		const savedMode = localStorage.getItem('mode-watcher-mode');
		if (mode.current === "dark" || savedMode === "dark") {
			isDarkMode = true;
		} else {
			isDarkMode = false;
		}
	});

</script>

<Dialog.Root open={open}
	onOpenChange={(isOpen) => {
		if (!isOpen && hasUnsavedChanges()) {
			showUnsavedChangesDialog = true;
		} else {
			open = isOpen;
		}
	}}
>
	<Dialog.Content
		class={`flex max-h-[85vh] z-100 sm:max-h-[90vh] p-3 sm:p-4 md:p-4 pb-6! flex-col min-w-[95vw] lg:min-w-[85vw]
			gap-0 overflow-hidden bg-background/95 dark:bg-background/60 supports-backdrop-filter:bg-background/90 dark:supports-backdrop-filter:bg-background/60 backdrop-blur-xl`}
		showCloseButton={false}
		onInteractOutside={(e) => {
			if (hasUnsavedChanges()) {
				e.preventDefault();
				showUnsavedChangesDialog = true;
			}
		}}
		onEscapeKeydown={(e) => {
			if (hasUnsavedChanges()) {
				e.preventDefault();
				showUnsavedChangesDialog = true;
			}
		}}
	>

		<div class="sm:px-4 sm:py-2 md:flex md:items-center md:justify-between shrink-0
			relative after:absolute after:bottom-0 after:left-0 after:h-[1px] after:w-full after:bg-gradient-to-r
			after:from-transparent after:via-sidebar-border after:to-transparent"
		>
			<div class="flex items-center justify-between px-0.5 gap-x-2 sm:gap-3 max-md:mb-4">
				<button
					data-sidebar-toggle
					class="lg:hidden p-1.5 z-50 cursor-pointer"
					onclick={(e) => {
						e.stopPropagation();
						sidebarOpen = !sidebarOpen;
					}}
				>
					<HugeiconsIcon
						icon={SidebarLeftIcon}
						class="size-5"
					/>
				</button>
				<div class="mt-3 max-md:text-center">
					<Dialog.Title
						class="sm:text-lg text-[18px] font-semibold">
						Global configs...
					</Dialog.Title>
					<Dialog.Description
						class="text-sm">
							Manage global options and daemon arguments
					</Dialog.Description>
				</div>
				<div class="md:hidden" >
					<button
						type="button"
						class=" cursor-pointer"
						onclick={() => mobileSearchOpen = true}
					>
						<HugeiconsIcon
							icon={Search01Icon}
							class="size-5"
						/>
					</button>
				</div>
			</div>

			<div class="relative lg:w-full lg:max-w-sm hidden md:block"
				bind:this={searchContainer}
			>
				<div class="relative">
					<HugeiconsIcon
						icon={Search01Icon}
						class="absolute left-3 top-1/2 -translate-y-1/2 size-4 text-muted-foreground"
					/>
					<input
						type="text"
						bind:value={searchQuery}
						placeholder="Search settings..."
						class="w-full rounded-full border border-sidebar-border/80 bg-muted/40 pl-8 pr-4 py-1.5 text-sm outline-none
							focus:ring-1 focus:ring-primary/30"
					/>
				</div>

				{#if searchResults.length > 0}
					<div
						transition:fly={{ y: -10, duration: 400 }}
						class="absolute top-full mt-2 w-full max-h-80 flex flex-col rounded-xl border border-sidebar-border
							bg-popover/90 backdrop-blur-2xl shadow-sm z-50 p-1"
					>
					<div class="overflow-y-auto overflow-x-hidden p-1 flex-1">
						{#each searchResults as result (result)}
							<button
								onclick={() => jumpToField(result.sectionId, result.key)}
								class="w-full border-b border-sidebar-border h-16 flex flex-row items-center gap-x-2 gap-y-1 text-left px-3 py-2 text-sm
									hover:cursor-pointer rounded-xl hover:bg-muted-foreground/10 dark:hover:bg-sidebar-accent hover:text-sidebar-foreground"
							>
								<div>
									<img
										src={isDarkMode ? OptionsLight : Options}
										alt="svg-icon"
										aria-hidden="true"
										class="w-5 shrink-0"
									/>
								</div>
								<div class="flex flex-col">
									<span class="font-medium text-sm text-primary">
										{result.key}
									</span>
									<span class="text-xs font-light overflow-hidden text-muted-foreground">
										In {truncateText(settingsSections.find(s => s.id === result.sectionId)?.title.toLowerCase(), 40)}
									</span>
								</div>
							</button>
						{/each}
					</div>
					<div
						class="h-8 dark:bg-sidebar-accent/30 bg-sidebar-accent rounded-xl backdrop-blur-sm border
						border-sidebar-border/80 dark:border-sidebar-border/40 px-3 flex items-center shrink-0 w-full"
					>
						<span class="text-xs font-medium text-muted-foreground">
							Results: {searchResults.length}
						</span>
					</div>
					</div>
				{/if}
			</div>

		</div>

		<div class="flex min-h-0 lg:max-h-[65svh] flex-1 relative">
			<aside
				bind:this={sidebarContainer}
				class={`absolute inset-y-0 max-sm:ml-[-15px] rounded-3xl max-lg:ml-[-25px] left-0 w-64 transform
					transition-transform duration-300 ease-in-out lg:relative lg:translate-x-0 z-800 border-r
					bg-background/95 dark:bg-background/60! supports-backdrop-filter:bg-background/90!
					dark:supports-backdrop-filter:bg-background/60 backdrop-blur-xl border-sidebar-border/60 blur-out-xs
					${sidebarOpen ? "translate-x-[0px]" : "-translate-x-[100%]"}`}
			>
				<div
					class="p-3 space-y-1 overflow-y-auto h-full z-20">
					{#each settingsSections as section (section)}
						<button
							type="button"
							title={section.title}
							onclick={() => {
								activeSectionId = section.id;
								sidebarOpen = window.innerWidth >= 1024;
							}}
							class={`w-full cursor-pointer flex items-center gap-3 rounded-2xl px-3 py-2.5 text-left transition-colors
								${activeSectionId === section.id ? "bg-primary/10 text-primary font-medium" : "text-muted-foreground hover:bg-sidebar-accent hover:text-sidebar-foreground"}`
							}
						>
							<img
								src={resolveIcon(section.icon)}
								alt="settings-icon"
								class="size-4 shrink-0"
							/>
							<span class="text-sm truncate">
								{section.title}
							</span>
						</button>
					{/each}
				</div>
			</aside>

			<div class="flex-1 h-[65svh]! overflow-y-auto p-2 md:p-4 sm:p-6 relative">
				{#if isFetching}
					<div class="absolute h-full text-[16px] inset-0 z-10 flex items-center justify-center backdrop-blur-xl"
						in:fly={{ y: 20, duration: 300, delay: 0, easing: quintOut }}
						out:fade={{ duration: 300 }}
					>
						<span class="text-muted-foreground animate-pulse">
							Loading configurations...
						</span>
					</div>
				{/if}

				{#if activeSection && !isFetching}
					{#key activeSectionId}
						<div
							class={`mx-auto space-y-6 pb-8 ${sidebarOpen ? 'blur-sm opacity-50 lg:blur-none lg:opacity-100' : ''}`}
							in:fly={{ y: 20, duration: 500, delay: sectionTransitionDelay, easing: quintOut }}
							out:fade={{ duration: 300 }}
						>
							<div>
								<h2 class="md:text-2xl font-semibold text-lg">
									{activeSection.title}
								</h2>
								<p class="md:text-sm text-xs text-muted-foreground md:mt-1">
									{activeSection.description}
								</p>
							</div>

							<div class="grid gap-4 md:grid-cols-2">
								{#each activeSection.fields as field (field.key)}
									<GlobalOptionsFields
										{field}
										bind:globalDraft={workingDraft.global}
										bind:cmdDraft={workingDraft.cmd}
									/>
								{/each}
							</div>
						</div>
					{/key}
				{/if}
			</div>
		</div>

    <Dialog.Footer class="mt-2 shrink-0 gap-6 rounded-3xl max-sm:py-3 z-50 flex flex-row items-center justify-evenly sm:justify-between lg:justify-end
				bg-background/95 dark:bg-background/60 supports-backdrop-filter:bg-background/90
				dark:supports-backdrop-filter:bg-background/60 backdrop-blur-xl border-sidebar-border/60 blur-out-xs"
			>
				<div class="text-xs flex-row text-muted-foreground/70 mr-[24%] hidden lg:flex">
					<HugeiconsIcon
						icon={InformationCircleIcon}
						class="size-4 mr-1"
					/>
					<span>Some changes requires daemon to restart!!</span>
				</div>
				<Button
					variant="outline"
					onclick={() => {
						if (hasUnsavedChanges()) {
							showUnsavedChangesDialog = true;
						} else {
							open = false;
						}
					}}
					disabled={isSaving}
					class="cursor-pointer max-sm:text-xs min-w-[100px] sm:min-w-[160px]"
				>
					Cancel
				</Button>
				<Button
					onclick={async () => {
						const success = await save();
						if (success) open = false;
					}}
					disabled={isSaving || isFetching}
					class="sm:min-w-[160px] min-w-[100px] cursor-pointer max-sm:text-xs"
				>
					{isSaving ? "Applying..." : "Save changes"}
				</Button>
    </Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root
	bind:open={showUnsavedChangesDialog}
>
  <Dialog.Overlay class="fixed  inset-0 z-200 backdrop-blur-xl" />
  <Dialog.Content
		class="sm:max-w-lg z-250 min-h-64  border border-sidebar-border/60 bg-background/95 dark:bg-background/60
			supports-backdrop-filter:bg-background/90 dark:supports-backdrop-filter:bg-background/60 backdrop-blur-xl"
		showCloseButton={false}
		>
    <Dialog.Header>
      <Dialog.Title class="sm:text-lg text-[18px] font-semibold">
				Unsaved changes...
			</Dialog.Title>
      <Dialog.Description>
				You have unsaved changes
				<br/>
				Save them before closing or discard them
			</Dialog.Description>
    </Dialog.Header>
    <div class="mt-4 flex flex-row max-sm:hidden flex-wrap justify-evenly md:justify-end gap-2">
      <Button
        variant="destructive"
        onclick={() => {
          workingDraft = cloneDraft(initialDraft);
          showUnsavedChangesDialog = false;
          open = false;
        }}
        class="cursor-pointer max-sm:w-32 max-sm:text-[14px]"
      >
        Discard changes
      </Button>
      <Button
				variant="outline"
				onclick={() => {
					open = true
					showUnsavedChangesDialog = false
				}}
				class="cursor-pointer max-sm:w-32 max-sm:text-[14px]">
				Keep editing
			</Button>
      <Button
        onclick={async () => {
          const saved = await save();
          if (saved) {
            showUnsavedChangesDialog = false;
						open = false
          }
        }}
        disabled={isSaving}
        class="cursor-pointer min-w-32 max-sm:w-32 max-sm:text-[14px]"
      >
        {isSaving ? "Saving..." : "Save & close"}
      </Button>
    </div>
		<!-- For mobile devices -->
		<div class="w-full! flex justify-center">
			<Button
				variant="outline"
				onclick={() => {
					open = true
						showUnsavedChangesDialog = false
					}}
				class="cursor-pointer max-sm:w-32 sm:hidden justify-center flex w-[90%]! max-sm:text-[14px]">
			Keep editing
			</Button>
		</div>
    <div class="mt-4 flex flex-row flex-wrap sm:hidden justify-evenly md:justify-end gap-2">
      <Button
        variant="destructive"
        onclick={() => {
          workingDraft = cloneDraft(initialDraft);
          showUnsavedChangesDialog = false;
          open = false;
        }}
        class="cursor-pointer max-sm:w-32 max-sm:text-[14px]"
      >
        Discard changes
      </Button>
      <Button
        onclick={async () => {
          const saved = await save();
          if (saved) {
            showUnsavedChangesDialog = false;
						open = false
          }
        }}
        disabled={isSaving}
        class="cursor-pointer min-w-32 max-sm:w-32 max-sm:text-[14px]"
      >
        {isSaving ? "Saving..." : "Save & close"}
      </Button>
    </div>
  </Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={mobileSearchOpen}>
  <Dialog.Overlay class="fixed inset-0 z-[150] backdrop-blur-xl" />
  <Dialog.Content
		class="flex flex-col z-[200] justify-center items-center p-0 gap-0 sm:max-w-lg bg-background/95 backdrop-blur-xl
		border border-sidebar-border/60 w-auto md:hidden rounded-2xl"
		showCloseButton={false}
	>

    <div class="flex justify-center items-center px-4 py-3 border-b border-sidebar-border/60">
      <HugeiconsIcon icon={Search01Icon} class="size-5 text-muted-foreground mr-3" />
      <input
        type="text"
        bind:value={searchQuery}
        placeholder="Search settings..."
        class="w-[60vw] bg-transparent text-sm outline-none placeholder:text-muted-foreground"
      />
    </div>

    <div class="overflow-y-auto max-h-[30vh] w-[80vw] p-2">
      {#if searchResults.length > 0}
        {#each searchResults as result (result)}
          <button
            onclick={() => jumpToField(result.sectionId, result.key)}
            class="w-full border-b border-sidebar-border/30 last:border-0 h-16 flex flex-row items-center
							gap-x-3 text-left px-3 py-2 text-sm hover:bg-muted-foreground/10 rounded-xl"
          >
            <div class="bg-muted p-2 rounded-md">
              <img src={isDarkMode ? OptionsLight : Options} alt="svg-icon" aria-hidden="true" class="w-4 shrink-0" />
            </div>
            <div class="flex flex-col">
              <span class="font-medium max-xs:text-xs text-sm text-primary">{result.key}</span>
              <span class="text-xs font-light text-muted-foreground">
                In {truncateText(settingsSections.find(s => s.id === result.sectionId)?.title.toLowerCase(), 30)}
              </span>
            </div>
          </button>
        {/each}
      {:else if searchQuery.trim() !== ""}
        <div class="py-8 max-xs:text-xs text-center text-sm text-muted-foreground">
          No settings found for "{searchQuery}"
        </div>
      {:else}
        <div class="py-8 text-center max-xs:text-xs text-sm text-muted-foreground">
          Type to search global options...
        </div>
      {/if}
    </div>
  </Dialog.Content>
</Dialog.Root>


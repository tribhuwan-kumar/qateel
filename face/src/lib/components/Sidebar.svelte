<script lang="ts">
  import { onMount, type Snippet } from "svelte";
	import { toast } from "svelte-sonner";
	import { mode } from "mode-watcher";
  import { systemState } from "$lib/system";
  import { clearSelection } from "$lib/selection";
  import { HugeiconsIcon } from "@hugeicons/svelte";
	import * as Select from "$lib/components/ui/select";
	import { logout, authState } from "$lib/auth/auth.svelte";
	import Label from "$lib/components/ui/label/label.svelte";
	import ThemeToggle from "$lib/components/ThemeToggle.svelte";
	import { aria2, aria2Store } from "$lib/aria2/client.svelte";
	import * as Collapsible from "$lib/components/ui/collapsible";
	import * as Sidebar from "$lib/components/ui/sidebar/index.js";
	import * as DropdownMenu from "$lib/components/ui/dropdown-menu";
	import type { Aria2GlobalOptions } from "$lib/aria2/types";
	import GlobalOptions from "$lib/components/settings/GlobalOptions.svelte";
  import {
		downloadIcons,
		getItemCategory,
		selectedCategory,
    type DownloadCategory
	} from "$lib/categorize";
  import {
    SoftwareIcon,
    Folder01Icon,
    ArrowUp01Icon,
    HelpCircleIcon,
    Settings01Icon,
    ArrowDown01Icon,
    BitcoinCircleIcon
  } from "@hugeicons/core-free-icons";

  let { children }: { children?: Snippet } = $props();

  let timer: ReturnType<typeof setTimeout> | undefined;
  let isDarkMode = $state(false);
  let showMoreDialog = $state(false);
	let username = $derived(authState.username);
	let role = $derived(authState.role);
  let isAria2Alive = $derived($systemState.status?.aria2Alive);

  type CategoryItem = {
    key: DownloadCategory;
    label: string;
    iconKey: keyof typeof downloadIcons;
  };

  const typeItems: CategoryItem[] = [
    { key: "everything", label: "Everything", iconKey: "everything" },
    { key: "torrent", label: "Torrent", iconKey: "torrent" },
    { key: "music", label: "Music", iconKey: "music" },
    { key: "photos", label: "Photos", iconKey: "photos" },
    { key: "programs", label: "Programs", iconKey: "programs" },
    { key: "videos", label: "Videos", iconKey: "videos" },
    { key: "subtitles", label: "Subtitles", iconKey: "subtitles" },
    { key: "compressed", label: "Compressed", iconKey: "compressed" },
    { key: "others", label: "Others", iconKey: "other" }
  ];

  type RangeSettingKey =
    | "maxConcurrentDownloads"
    | "maxOverallDownloadLimit"
    | "maxOverallUploadLimit";

  type RangeControl = {
    key: RangeSettingKey;
    optionKey: keyof Aria2GlobalOptions;
    label: string;
    min: number;
    max: number;
    step: number;
    encode: (value: number) => string;
    format: (value: number) => string;
  };

  const rangeControls: RangeControl[] = [
    {
      key: "maxConcurrentDownloads",
      optionKey: "max-concurrent-downloads",
      label: "Max concurrent downloads",
      min: 1,
      max: 1000,
      step: 1,
      encode: (value) => String(value),
      format: (value) => String(value)
    },
    {
      key: "maxOverallDownloadLimit",
      optionKey: "max-overall-download-limit",
      label: "Max overall download limit",
      min: 0,
      max: 16384,
      step: 8,
      encode: (value) => encodeSpeed(value),
      format: (value) => formatSpeed(value)
    },
    {
      key: "maxOverallUploadLimit",
      optionKey: "max-overall-upload-limit",
      label: "Max overall upload limit",
      min: 0,
      max: 16384,
      step: 8,
      encode: (value) => encodeSpeed(value),
      format: (value) => formatSpeed(value)
    }
  ];

  const textControls = [
    { key: "dir", optionKey: "dir", label: "Dir" },
    { key: "log", optionKey: "log", label: "Log" }
  ] as const;

  // Unified reactive state object
  let quickSettings = $state({
    log: "",
		dir: "",
    logLevel: "",
    saveSession: "",
    maxConcurrentDownloads: 5,
    maxOverallDownloadLimit: 0,
    maxOverallUploadLimit: 0
  });

	let itemMetadataStore = $derived(aria2Store.items);

  let categoryCounts = $derived.by(() => {
    const counts: Record<DownloadCategory, number> = {
      everything: itemMetadataStore.length,
      torrent: 0,
      music: 0,
      photos: 0,
      programs: 0,
      videos: 0,
      subtitles: 0,
      compressed: 0,
      others: 0
    };

    for (const item of itemMetadataStore) {
      const category = getItemCategory(item);
      counts[category] += 1;
    }

    return counts;
  });

  const encodeSpeed = (val: number): string => (val === 0 ? "0" : `${val}K`);

  const decodeSpeed = (val: string | undefined): number => {
    if (!val || val === "0") return 0;
    let multiplier = 1;
    const upper = val.toUpperCase();
    if (upper.endsWith("M")) multiplier = 1024;
    else if (!upper.endsWith("K")) multiplier = 1 / 1024;
    const parsed = Number(val.replace(/[^0-9.]/g, ""));
    return Number.isFinite(parsed) ? Math.round(parsed * multiplier) : 0;
  };

  async function loadGlobalOptions() {
    try {
      const options = await aria2.getGlobalOption();
      quickSettings.log = options["log"] ?? "";
      quickSettings.dir = options["dir"] ?? "";
      quickSettings.logLevel = options["log-level"] ?? "notice";
      quickSettings.saveSession = options["save-session"] ?? "";
      quickSettings.maxConcurrentDownloads = Number(options["max-concurrent-downloads"] ?? 5);

      // Decode raw aria strings ("5M", "100K", "0") into KiB/s
      quickSettings.maxOverallDownloadLimit = decodeSpeed(options["max-overall-download-limit"]);
      quickSettings.maxOverallUploadLimit = decodeSpeed(options["max-overall-upload-limit"]);
    } catch (error) {
      console.error("Failed to load global options:", error);
      toast.error("Failed to load global options:", {
				description: error as string,
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }

  function selectCategory(category: DownloadCategory) {
    selectedCategory.set(category);
    clearSelection();
  }

  const formatSpeed = (val: number): string => {
    if (val === 0) return "Unlimited";
    if (val < 1024) return `${val} KiB/s`;
    return `${(val / 1024).toFixed(2).replace(/\.00$/, "")} MiB/s`;
  };

  async function pushSetting(key: keyof Aria2GlobalOptions, value: string) {
    try {
      await aria2.changeGlobalOption({ [key]: value });
    } catch (error) {
      toast.error(`Failed to update option: ${key}`, {
				description: error as string,
        richColors: true,
        style: "cursor: pointer;"
      });
      console.error(`Failed to update option ${key}:`, error);
      await loadGlobalOptions();
    }
  }

  function handleRangeInput(control: RangeControl, value: number) {
    quickSettings[control.key] = value;
		pushSetting(control.optionKey, control.encode(value));
  }

  const getCategoryIcon = (item: CategoryItem) =>
    downloadIcons[item.iconKey][isDarkMode ? "dark" : "light"];

  onMount(async () => {
    const update = () => {
			const savedMode = localStorage.getItem('mode-watcher-mode')
			if (mode.current === "dark" || savedMode === "dark") {
				isDarkMode = true;
			} else {
				isDarkMode = false;
			}
    };
    update();
    loadGlobalOptions();
  });
</script>

<Sidebar.Root>
  <Sidebar.Header class="flex flex-row font-medium items-baseline max-xs:text-[18px] text-[20px]">
    Qateel
    <span class="inline text-[14px] font-normal italic">
      biblically accurate aria2 web ui
    </span>
  </Sidebar.Header>

  <Sidebar.Content>
    <div class="px-2 py-3 space-y-4">
      <Sidebar.Menu>
        <Collapsible.Root open class="group/collapsible">
          <Sidebar.MenuItem>
            <Collapsible.Trigger>
              {#snippet child({ props })}
                <Sidebar.MenuButton {...props} class="w-full cursor-pointer justify-between">
                  <div class="flex items-center gap-2">
                    <HugeiconsIcon icon={Folder01Icon} strokeWidth={2} class="size-4" />
                    <span class="font-medium">Category</span>
                  </div>
                  <HugeiconsIcon
                    icon={ArrowDown01Icon}
                    strokeWidth={2}
                    class="size-4 shrink-0 transition-transform duration-200 group-data-[state=open]/collapsible:rotate-180"
                  />
                </Sidebar.MenuButton>
              {/snippet}
            </Collapsible.Trigger>
            <Collapsible.Content>
              <Sidebar.MenuSub>
                {#each typeItems as item (item.key)}
                  <Sidebar.MenuSubItem>
                    <Sidebar.MenuSubButton
                      onclick={() => selectCategory(item.key)}
                      class={$selectedCategory === item.key ? "bg-sidebar-accent text-sidebar-accent-foreground cursor-pointer" : "cursor-pointer text-muted-foreground"}
                    >
                      <img src={getCategoryIcon(item)} alt={item.label} class="size-4 shrink-0" />
                      <span class={$selectedCategory === item.key ? "text-primary font-medium" : "text-muted-foreground"}>
                        {item.label}
                      </span>
                      <span class={$selectedCategory === item.key ? "text-primary ml-auto text-xs font-semibold" : "ml-auto text-xs text-muted-foreground opacity-70"}>
                        {categoryCounts[item.key] ?? 0}
                      </span>
                    </Sidebar.MenuSubButton>
                  </Sidebar.MenuSubItem>
                {/each}
              </Sidebar.MenuSub>
            </Collapsible.Content>
          </Sidebar.MenuItem>
        </Collapsible.Root>

        <Collapsible.Root open class="group/collapsible">
          <Sidebar.MenuItem>
            <Collapsible.Trigger>
              {#snippet child({ props })}
                <Sidebar.MenuButton {...props} class="w-full cursor-pointer justify-between mt-2">
                  <div class="flex items-center gap-2">
                    <HugeiconsIcon icon={Settings01Icon} strokeWidth={2} class="size-4" />
                    <span class="font-medium">Global configs</span>
                  </div>
                  <HugeiconsIcon
                    icon={ArrowDown01Icon}
                    strokeWidth={2}
                    class="size-4 shrink-0 transition-transform duration-200 group-data-[state=open]/collapsible:rotate-180"
                  />
                </Sidebar.MenuButton>
              {/snippet}
            </Collapsible.Trigger>

            <Collapsible.Content>
              <Sidebar.MenuSub>
                <div class="space-y-3 pt-1">
                  {#each rangeControls as control (control.key)}
                    <div class={`rounded-xl border border-sidebar-border bg-sidebar-background/80 p-3 space-y-2 ${!isAria2Alive ? "cursor-not-allowed!" : ""}`}>
                      <div class="flex items-center justify-between">
                        <Label class="block text-xs font-medium tracking-wide text-sidebar-foreground/70">
                          {control.label}
                        </Label>
                        <span class="text-xs text-muted-foreground">
                          {control.format(quickSettings[control.key])}
                        </span>
                      </div>
                      <input
                        type="range"
                        disabled={!isAria2Alive}
                        class={`cursor-pointer custom-slider ${!isAria2Alive ? "cursor-not-allowed!" : ""}`}
                        min={control.min}
                        max={control.max}
                        step={control.step}
                        value={quickSettings[control.key]}
                        oninput={(e) => {
													quickSettings[control.key] = Number(e.currentTarget.value);
                        }}
												onchange={(e) => {
													const val = Number(e.currentTarget.value);
													handleRangeInput(control, val);
												}}
                      />
                    </div>
                  {/each}

                  {#each textControls as control (control.key)}
                    <div class="rounded-xl border border-sidebar-border bg-sidebar-background/80 p-3 space-y-2">
                      <Label class="block text-xs font-medium tracking-wide text-sidebar-foreground/70">
                        {control.label}
                      </Label>
                      <input
                        type="text"
                        value={quickSettings[control.key]}
                        onchange={(e) => {
                          quickSettings[control.key] = e.currentTarget.value;
                          pushSetting(control.optionKey, e.currentTarget.value);
                        }}
                        class={`w-full rounded-md border border-sidebar-border bg-sidebar-background/80 px-3 py-1.5 text-sm outline-none placeholder:text-muted-foreground/50 text-foreground transition-shadow focus-visible:ring-1 focus-visible:ring-sidebar-border ${!isAria2Alive ? "cursor-not-allowed!" : ""}`}
                        disabled={!isAria2Alive}
                      />
                    </div>
                  {/each}

                  <div class="rounded-xl border border-sidebar-border bg-sidebar-background/80 p-3 space-y-2">
                    <Label class="block text-xs font-medium tracking-wide text-sidebar-foreground/70">
                      Log level
                    </Label>
                    <Select.Root
                      type="single"
                      value={quickSettings.logLevel}
                      disabled={!isAria2Alive}
                      onValueChange={(value: string) => {
                        if (value) {
                          quickSettings.logLevel = value;
                          pushSetting("log-level", value);
                        }
                      }}
                    >
                      <Select.Trigger
                        disabled={!isAria2Alive}
                        class="w-full rounded-md border cursor-pointer border-sidebar-border bg-sidebar-background/80 px-3 py-1.5 text-sm text-left flex justify-between items-center transition-colors hover:bg-muted/30"
                      >
                        {quickSettings.logLevel || "Select log level"}
                      </Select.Trigger>
                      <Select.Content class="bg-popover/50 shadow-md p-1 min-w-[8rem]">
                        <Select.Group>
                          <Select.Item value="debug" class="px-2 py-1 text-sm text-muted-foreground! hover:text-primary! cursor-pointer hover:bg-accent">debug</Select.Item>
                          <Select.Item value="info" class="px-2 py-1 text-sm cursor-pointer hover:bg-accent text-muted-foreground! hover:text-primary!">info</Select.Item>
                          <Select.Item value="notice" class="px-2 py-1 text-sm cursor-pointer hover:bg-accent text-muted-foreground! hover:text-primary!">notice</Select.Item>
                          <Select.Item value="warn" class="px-2 py-1 text-sm cursor-pointer hover:bg-accent text-muted-foreground! hover:text-primary!">warn</Select.Item>
                          <Select.Item value="error" class="px-2 py-1 text-sm cursor-pointer hover:bg-accent text-muted-foreground! hover:text-primary!">error</Select.Item>
                        </Select.Group>
                      </Select.Content>
                    </Select.Root>
                  </div>

                  <button
                    type="button"
                    onclick={() => {
											showMoreDialog = true;
										}}
										disabled={!isAria2Alive}
                    class={`w-full rounded-xl border border-sidebar-border bg-sidebar-background/80 px-3 py-3 text-left text-sm font-semibold transition-colors hover:bg-sidebar-accent hover:text-sidebar-accent-foreground ${!isAria2Alive ? "cursor-not-allowed!" : ""} cursor-pointer group`}
                  >
                    <span class="group-hover:text-primary transition-colors">
                      Global configs
                    </span>
                    <span class="block text-xs font-normal text-muted-foreground mt-0.5">
                      Set additional global configuration parameters.
                    </span>
                  </button>
                </div>
              </Sidebar.MenuSub>
            </Collapsible.Content>
          </Sidebar.MenuItem>
        </Collapsible.Root>

        <Collapsible.Root open class="group/collapsible">
          <Sidebar.MenuItem>
            <Collapsible.Trigger>
              {#snippet child({ props })}
                <Sidebar.MenuButton {...props} class="w-full cursor-pointer justify-between mt-2">
                  <div class="flex items-center gap-2">
                    <HugeiconsIcon icon={SoftwareIcon} strokeWidth={2} class="size-4" />
                    <span class="font-medium">App info</span>
                  </div>
                  <HugeiconsIcon
                    icon={ArrowDown01Icon}
                    strokeWidth={2}
                    class="size-4 shrink-0 transition-transform duration-200 group-data-[state=open]/collapsible:rotate-180"
                  />
                </Sidebar.MenuButton>
              {/snippet}
            </Collapsible.Trigger>
            <Collapsible.Content>
              <Sidebar.MenuSub>
                <Sidebar.MenuSubItem>
                  <Sidebar.MenuSubButton class="cursor-pointer text-muted-foreground!">
                    Aria2
                  </Sidebar.MenuSubButton>
                </Sidebar.MenuSubItem>
                <Sidebar.MenuSubItem>
                  <Sidebar.MenuSubButton class="cursor-pointer text-muted-foreground!">
                    Qateel
                  </Sidebar.MenuSubButton>
                </Sidebar.MenuSubItem>
                <Sidebar.MenuSubItem>
                  <Sidebar.MenuSubButton class="cursor-pointer text-muted-foreground!">
                    Storage
                  </Sidebar.MenuSubButton>
                </Sidebar.MenuSubItem>
              </Sidebar.MenuSub>
            </Collapsible.Content>
          </Sidebar.MenuItem>
        </Collapsible.Root>

        <div class="my-2 border-t border-sidebar-border"></div>

        <Sidebar.MenuItem>
          <Sidebar.MenuButton class="w-full justify-start cursor-pointer">
            <HugeiconsIcon icon={HelpCircleIcon} strokeWidth={2} class="size-4 mr-2" />
            <span>Help</span>
          </Sidebar.MenuButton>
        </Sidebar.MenuItem>

        <Sidebar.MenuItem>
          <Sidebar.MenuButton class="w-full justify-start cursor-pointer">
            <HugeiconsIcon icon={BitcoinCircleIcon} strokeWidth={2} class="size-4 mr-2" />
            <span>Donations</span>
          </Sidebar.MenuButton>
        </Sidebar.MenuItem>

        <div class="my-2 border-t border-sidebar-border"></div>

        <Sidebar.MenuItem class="pointer-events-none!">
          <Sidebar.MenuButton class="min-h-12 pointer-events-none! flex justify-center items-center cursor-pointer hover:bg-transparent!">
            <Sidebar.MenuSubItem class="pointer-events-none!">
              <ThemeToggle class="cursor-pointer pointer-events-auto" />
            </Sidebar.MenuSubItem>
          </Sidebar.MenuButton>
        </Sidebar.MenuItem>
      </Sidebar.Menu>
    </div>
  </Sidebar.Content>

  <Sidebar.Footer>
    <Sidebar.Menu>
      <Sidebar.MenuItem>
        <DropdownMenu.Root>
          <DropdownMenu.Trigger>
            {#snippet child({ props })}
              <Sidebar.MenuButton
                {...props}
								title={`'${username}' has '${role}' role`}
                class="w-full rounded-xl h-16! relative cursor-pointer border border-sidebar-border/60 bg-sidebar-background/80 px-3
									py-6 shadow-sm transition-colors data-[state=open]:bg-sidebar-accent data-[state=open]:text-sidebar-accent-foreground"
              >
                <div class="flex min-w-0 flex-1 items-center gap-3">
                  <div class="flex h-9 w-9 items-center justify-center rounded-full bg-[#DDA0DD] text-xs font-semibold text-sidebar-primary-foreground">
										{username?.charAt(0).toUpperCase()}
                  </div>
                  <div class="min-w-0 text-left">
                    <p class="text-[16px] font-medium text-primary">{username}</p>
										<span class="absolute top-9 italic right-[60px]">
											<span class="text-[12px] text-sidebar-foreground/90 font-light">as</span>
											<span class="text-sidebar-foreground/60 text-[14px]">{role}</span>
										</span>
                  </div>
                </div>
                <HugeiconsIcon
									icon={ArrowUp01Icon}
									strokeWidth={2}
									class="ml-2 size-4 shrink-0"
								/>
              </Sidebar.MenuButton>
            {/snippet}
          </DropdownMenu.Trigger>

          <DropdownMenu.Content side="top" align="center" class="w-(--bits-dropdown-menu-anchor-width) bg-popover/50 p-2 pointer-events-none!">
            <DropdownMenu.Group>
              <DropdownMenu.Label class="px-2 py-1.5 mt-1 text-xs font-semibold text-muted-foreground pointer-events-auto">
                User info
              </DropdownMenu.Label>
              <DropdownMenu.Item class="flex hover:rounded-xl items-center px-2 py-1.5 text-sm rounded-md
								pointer-events-auto cursor-pointer hover:bg-accent/10! text-muted-foreground! hover:text-primary!">
                <span>
                  Account
                </span>
              </DropdownMenu.Item>
              <DropdownMenu.Item class="flex items-center hover:rounded-xl px-2 py-1.5 text-sm rounded-md
								pointer-events-auto cursor-pointer hover:bg-accent/10! text-muted-foreground! hover:text-primary!">
                <span>
                  Create user
                </span>
              </DropdownMenu.Item>
              <DropdownMenu.Item class="flex items-center hover:rounded-xl px-2 py-1.5 text-sm rounded-md
								pointer-events-auto cursor-pointer hover:bg-accent/10! text-muted-foreground! hover:text-primary!">
                <span>
									Settings
                </span>
              </DropdownMenu.Item>
            </DropdownMenu.Group>
            <DropdownMenu.Separator class="h-px bg-border my-1" />
            <DropdownMenu.Item
							onclick={logout}
							class="flex items-center hover:rounded-xl hover:bg-accent/10! px-2 py-1.5 pointer-events-auto text-sm rounded-md cursor-pointer
								text-destructive focus:bg-destructive/10 focus:text-destructive"
						>
              <span>Sign out</span>
            </DropdownMenu.Item>
          </DropdownMenu.Content>
        </DropdownMenu.Root>
      </Sidebar.MenuItem>
    </Sidebar.Menu>
  </Sidebar.Footer>
</Sidebar.Root>

<Sidebar.Inset>
  <div class="min-h-screen bg-gradient-to-br from-background via-background to-muted/20">
    {@render children?.()}
  </div>
</Sidebar.Inset>

<GlobalOptions bind:open={showMoreDialog} onsave={loadGlobalOptions} />


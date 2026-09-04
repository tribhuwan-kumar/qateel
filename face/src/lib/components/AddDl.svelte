<script lang="ts">
  import { toast } from "svelte-sonner";
  import { truncateMiddle } from "$lib/utils";
	import { aria2 } from '$lib/aria2/client.svelte';
  import { Input } from "$lib/components/ui/input";
  import { Label } from "$lib/components/ui/label";
  import { HugeiconsIcon } from "@hugeicons/svelte";
  import { Button } from "$lib/components/ui/button";
  import { Textarea } from "$lib/components/ui/textarea";
	import ProbeUriDialog from "$lib/components/ProbeUri.svelte";
  import {
    Tabs,
    TabsContent,
    TabsList,
    TabsTrigger
    } from "$lib/components/ui/tabs";
	import {
    EyeIcon,
    Link01Icon,
    Cancel01Icon,
    File02Icon,
    PlusSignIcon,
    FileUploadIcon,
    LoaderPinwheelIcon,
  } from '@hugeicons/core-free-icons';
  import {
    Dialog,
    DialogTitle,
    DialogFooter,
    DialogHeader,
    DialogContent,
    DialogDescription,
  } from "$lib/components/ui/dialog";

  let {
    open = $bindable(false),
    onclose,
  }: {
    open: boolean,
    onclose: () => void
  } = $props();

  let urls = $state("");
  let loading = $state(false);
  let activeTab = $state("link");
  let dragActive = $state(false);
  let selectedFiles = $state<File[]>([]);
  let showFileManageDialog = $state(false);
	let showProbeUriDialog = $state(false);
  let probeUris = $state<string[]>([]);

  async function handleDownload() {
    loading = true;
    try {
      if (activeTab === 'link') {
        const lines = urls.split('\n').map(u => u.trim()).filter(u => u.length > 0);
        const uniqueLines = [...new Set(lines)];

        if (uniqueLines.length === 0) return;

        if (lines.length > uniqueLines.length) {
          toast.info(`Removed ${lines.length - uniqueLines.length} duplicate links`, {
            richColors: true,
            style: "cursor: pointer;"
          });
        }
				probeUris = uniqueLines;
				showProbeUriDialog = true;
				open = false;
				urls = "";
      }
      else if (activeTab === 'torrent' && selectedFiles.length > 0) {
        const processedTorrents = await Promise.all(
          selectedFiles.map(async (file) => {
            const base64 = await toBase64(file);
            const cleanBase64 = base64.split(',')[1];
            return {
              torrent: cleanBase64,
              options: {}                                   /* Later utilize it maybe !? */
            };
          })
        );
        console.log("Adding batch torrents:", { torrents: processedTorrents });
        await aria2.addTorrents(processedTorrents);
      }

      // Reset state
      urls = "";
      selectedFiles = [];
      onclose();

    } catch (e) {
      console.log("add uri error:", e);
    } finally {
      loading = false;
    }
  }

  const toBase64 = (file: File) => new Promise<string>((resolve, reject) => {
    const reader = new FileReader();
    reader.readAsDataURL(file);
    reader.onload = () => resolve(reader.result as string);
    reader.onerror = error => reject(error);
  });

  function handleDrag(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    if (e.type === "dragenter" || e.type === "dragover") {
      dragActive = true;
    } else if (e.type === "dragleave") {
      dragActive = false;
    }
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    e.stopPropagation();
    dragActive = false;

    if (e.dataTransfer?.files) {
      addFiles(Array.from(e.dataTransfer.files));
    }
  }

  function handleFileSelect(e: Event) {
    const target = e.target as HTMLInputElement;
    if (target.files) {
      addFiles(Array.from(target.files));
    }
    /* Reset input value so same files can be selected again if needed */
    target.value = '';
  }

  function addFiles(files: File[]) {
    const validFiles = files.filter(f => f.name.toLowerCase().endsWith('.torrent'));
    const invalidCount = files.length - validFiles.length;

    const currentNames = new Set(selectedFiles.map(f => f.name));
    const uniqueNewFiles = validFiles.filter(f => !currentNames.has(f.name));
    const duplicateCount = validFiles.length - uniqueNewFiles.length;

    if (invalidCount > 0) {
      toast.warning(`Skipped ${invalidCount} non-torrent files!!`, {
        richColors: true,
        style: "cursor: pointer;"
      });
    };

    if (duplicateCount > 0) {
      toast.info(`Skipped ${duplicateCount} duplicate torrents!!`, {
        richColors: true,
        style: "cursor: pointer;"
      });
    }

    if (uniqueNewFiles.length > 0) {
      selectedFiles = [...selectedFiles, ...uniqueNewFiles];
      toast.success(`Selected ${uniqueNewFiles.length} torrents`, {
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }

  function removeFile(index: number) {
    selectedFiles = selectedFiles.filter((_, i) => i !== index);
    if (selectedFiles.length === 0) showFileManageDialog = false;
  }

</script>

<Dialog bind:open={open} onOpenChange={(v) => !v && onclose()}>
  <DialogContent class="sm:max-w-150 sm:h-120" showCloseButton={false}>
    <DialogHeader>
      <DialogTitle>Add download</DialogTitle>
      <DialogDescription>
        Paste links or upload <span class="italic">.torrent</span> files
      </DialogDescription>
    </DialogHeader>

    <Tabs bind:value={activeTab} class="sm:w-full sm:h-72">
      <TabsList class="grid w-full grid-cols-2">
        <TabsTrigger value="link" class="cursor-pointer!">
          <HugeiconsIcon
            icon={Link01Icon}
            class="w-4 h-4"
            strokeWidth={2}
            size={24}
          />
        </TabsTrigger>
        <TabsTrigger value="torrent" class="cursor-pointer!">
          <HugeiconsIcon
            icon={File02Icon}
            class="w-4 h-4"
            strokeWidth={2}
            size={24}
          />
        </TabsTrigger>
      </TabsList>

      <TabsContent value="link" class="space-y-4 py-4">
        <div class="space-y-2">
           <Label>Download URLs</Label>
           <Textarea
             placeholder="https://... magnet:?xt=... ftp://... sftp://... "
             class="h-[150px] sm:min-h-[150px] sm:max-h-[240px] text-xs"
             bind:value={urls}
           />
           <p class="text-xs text-muted-foreground">Supports multiple URIs</p>
        </div>
      </TabsContent>

      <TabsContent value="torrent" class="py-4 space-y-4">
        <button
          type="button"
          class="border-2 mt-[14px] border-dashed rounded-lg min-h-[150px] w-full flex flex-col items-center justify-center text-center p-4 transition-colors cursor-pointer bg-transparent
          {dragActive ? 'border-primary bg-primary/5' : 'border-muted-foreground/25'}"
          ondragenter={handleDrag}
          ondragleave={handleDrag}
          ondragover={handleDrag}
          ondrop={handleDrop}
          aria-label="drag and drop the .torrent files"
          onclick={() => document.getElementById('torrent-upload')?.click()}
        >
          <HugeiconsIcon
            icon={FileUploadIcon}
            class="w-8 h-8"
            strokeWidth={2}
            size={24}
          />
          <p class="text-sm font-medium mt-2">Drag and drop .torrent files</p>
          <p class="text-xs text-muted-foreground mt-1">Click to browse</p>
          <input
            class="hidden"
            type="file"
            id="torrent-upload"
            accept=".torrent"
            onchange={handleFileSelect}
            multiple={true}
          />
        </button>
        <p class="text-xs text-muted-foreground mt-1">You can add multiple files at once</p>
        {#if selectedFiles.length > 0}
          <div class="flex items-center justify-between bg-secondary/30 p-2 rounded-md animate-in fade-in slide-in-from-top-2">
            <span class="text-xs font-medium pl-2">{selectedFiles.length} files selected</span>
            <Button
              size="sm"
              variant="ghost"
              class="h-auto cursor-pointer py-1 px-2 text-xs"
              onclick={() => showFileManageDialog = true}
            >
              <HugeiconsIcon
                icon={EyeIcon}
                class="w-4 h-4"
                strokeWidth={2}
                size={24}
              />
            </Button>
          </div>
        {/if}
      </TabsContent>
    </Tabs>

    <DialogFooter class="max-md:flex max-md:flex-col gap-4">
      <Button
				variant="outline"
				onclick={onclose}
				disabled={loading}
				class="cursor-pointer min-w-24"
			>
				Cancel
			</Button>
      <Button
        onclick={handleDownload}
				class="cursor-pointer min-w-32 disabled:cursor-not-allowed"
        disabled={
        loading ||
          (activeTab === 'link' && !urls) ||
          (activeTab === 'torrent' && selectedFiles.length === 0)
        }
      >
        {#if loading}
          <HugeiconsIcon
            icon={LoaderPinwheelIcon}
            class="w-4 h-4"
            strokeWidth={2}
            size={24}
          />
        {:else}
          Continue
        {/if}
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>

<Dialog bind:open={showFileManageDialog}>
  <DialogContent class="sm:min-w-[400px]">
    <DialogHeader>
      <DialogTitle>Selected files</DialogTitle>
    </DialogHeader>

    <div class="sm:h-[350px] overflow-y-auto border rounded-md p-1 space-y-1">
      {#each selectedFiles as file, i (file.name + i)}
        <div class="flex items-center justify-between text-xs bg-muted/30 p-2 rounded hover:bg-muted/50 transition-colors group">
          <div class="flex items-center gap-2 truncate">
            <HugeiconsIcon
              icon={File02Icon}
              class="w-4 h-4"
              strokeWidth={2}
              size={24}
            />
            <span class="truncate">{truncateMiddle(file.name, 50)}</span>
          </div>
          <Button
            variant="ghost"
            size="icon"
            class="h-6 w-6 text-muted-foreground hover:text-destructive cursor-pointer opacity-0 group-hover:opacity-100 transition-opacity"
            onclick={() => removeFile(i)}
          >
            <HugeiconsIcon
              icon={Cancel01Icon}
              class="w-4 h-4"
              strokeWidth={2}
              size={24}
            />
          </Button>
        </div>
      {/each}
    </div>

    <DialogFooter class="flex flex-row justify-between sm:justify-between gap-2">
      <div class="relative">
        <Button
          title="add more!?"
          variant="ghost"
          size="sm"
          class="gap-1"
        >
          <HugeiconsIcon
            icon={PlusSignIcon}
            class="w-4 h-4"
            strokeWidth={2}
            size={24}
          />
        </Button>
        <Input
          title="add more!?"
          type="file"
          accept=".torrent"
          class="absolute inset-0 opacity-0 cursor-pointer"
          onchange={handleFileSelect}
          multiple
        />
      </div>

      <Button size="sm" onclick={() => showFileManageDialog = false}>
        Done
      </Button>
    </DialogFooter>
  </DialogContent>
</Dialog>

<ProbeUriDialog
  bind:open={showProbeUriDialog}
  uris={probeUris}
  onclose={() => showProbeUriDialog = false}
/>


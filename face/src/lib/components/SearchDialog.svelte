<script lang="ts">
  import { onMount } from "svelte";
  import * as Dialog from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Label } from "$lib/components/ui/label/index.js";
  import { toast } from "svelte-sonner";
  import type { ItemMetaData } from "$lib/aria2/types";
  import { aria2 } from "$lib/aria2/client.svelte";

  export let open: boolean = false;

  let q: string = "";
  let fileType: string = "";
  let extension: string = "";
  let page: number = 1;
  let perPage: number = 20;
  let total: number = 0;
  let results: ItemMetaData[] = [];
  let loading: boolean = false;
  let error: string | null = null;

  const fileTypeOptions = [
    "",
    "image",
    "video",
    "audio",
    "document",
    "archive",
    "book",
    "code",
    "subtitle",
    "font",
    "torrent",
    "playlist",
  ];

  function reset() {
    q = "";
    fileType = "";
    extension = "";
    page = 1;
    perPage = 20;
    total = 0;
    results = [];
    error = null;
  }

  async function doSearch(p = 1) {
    loading = true;
    error = null;

    try {
      const resp = await aria2.search({
        q: q || undefined,
        fileType: fileType || undefined,
        extension: extension || undefined,
        page: p,
        perPage: perPage,
      });
      results = resp.data ?? [];
      page = resp.page ?? 1;
      perPage = resp.perPage ?? perPage;
      total = resp.total ?? 0;
    } catch (e: any) {
      console.error("Search error", e);
      error = e?.message ?? String(e);
      toast.error("Search failed", { description: error });
    } finally {
      loading = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      open = false;
    }
    if (e.key === "Enter") {
      doSearch(1);
    }
  }

  onMount(() => {
    const handler = (ev: KeyboardEvent) => {
      if (open) return;
      // Ctrl+K or Slash to open quick search
      if ((ev.ctrlKey && ev.key.toLowerCase() === "k") || ev.key === "/") {
        ev.preventDefault();
        open = true;
      }
    };
    window.addEventListener("keydown", handler);
    return () => window.removeEventListener("keydown", handler);
  });
</script>

<Dialog.Root bind:open>
  <Dialog.Content class="sm:max-w-2xl">
    <Dialog.Header class="flex items-start justify-between">
      <div>
        <Dialog.Title>Search downloads</Dialog.Title>
        <Dialog.Description class="mt-1">Search and filter downloads by name, type or extension.</Dialog.Description>
      </div>
      <div class="flex items-center gap-2">
        <span class="text-xs text-muted-foreground">Quick: Ctrl+K or /</span>
        <button onclick={() => (open = false)} class="rounded-full p-1 hover:bg-muted/30">
          ✕
        </button>
      </div>
    </Dialog.Header>

    <div class="mt-4 space-y-3">
      <div class="grid grid-cols-1 md:grid-cols-3 gap-2 items-end">
        <div class="col-span-2">
          <Label for="q">Query</Label>
          <input
            id="q"
            class="w-full px-3 py-2 rounded border bg-input"
            type="text"
            bind:value={q}
            placeholder="Search by name, source URL or files... (press Enter to search)"
            onkeydown={onKeydown}
          />
        </div>

        <div>
          <Label for="fileType">File type</Label>
          <select id="fileType" bind:value={fileType} class="w-full px-3 py-2 rounded border bg-input">
            {#each fileTypeOptions as opt (opt)}
              <option value={opt}>{opt === "" ? "Any" : opt}</option>
            {/each}
          </select>
        </div>

        <div class="md:col-span-1 hidden"></div>

        <div class="col-span-1 md:col-span-1">
          <Label for="extension">Extension</Label>
          <input
            id="extension"
            class="w-full px-3 py-2 rounded border bg-input"
            type="text"
            bind:value={extension}
            placeholder="e.g. mkv, mp4, pdf"
            onkeydown={onKeydown}
          />
        </div>
      </div>

      <div class="flex items-center gap-2">
        <Button
					onclick={() => doSearch(1)}
					class="min-w-24"
					variant="default">
					{loading ? "Searching..." : "Search"}
				</Button>
        <Button
					onclick={() => { reset(); }}
					variant="outline">
					Reset
				</Button>
        <div class="ml-auto text-sm text-muted-foreground">Total: {total}</div>
      </div>

      <div class="mt-2">
        {#if error}
          <div class="text-destructive text-sm">{error}</div>
        {/if}

        {#if results.length === 0 && !loading}
          <div class="text-sm text-muted-foreground">No results</div>
        {/if}

        <ul class="space-y-2 mt-2">
          {#each results as item (item.gid)}
            <li class="p-2 border rounded hover:bg-muted/40">
              <div class="flex flex-col md:flex-row md:items-center md:justify-between gap-1">
                <div class="flex flex-col">
                  <div class="font-semibold">{item.name ?? "<Untitled>"}</div>
                  <div class="text-xs text-muted-foreground">{item.sourceUri ?? ""}</div>
                </div>
                <div class="text-xs text-muted-foreground md:text-right">
                  <div>Status: {item.status}</div>
                  <div>Size: {item.totalLength}</div>
                  <div>Added: {new Date(item.createdAt ?? "").toLocaleString()}</div>
                </div>
              </div>
            </li>
          {/each}
        </ul>

        {#if results.length > 0}
          <div class="flex items-center justify-between mt-4">
            <div class="text-sm">Showing {results.length} of {total}</div>
            <div class="flex items-center gap-2">
              <Button disabled={page <= 1} onclick={() => doSearch(page - 1)}>Previous</Button>
              <div class="text-sm">Page {page}</div>
              <Button disabled={(page * perPage) >= total} onclick={() => doSearch(page + 1)}>Next</Button>
            </div>
          </div>
        {/if}
      </div>
    </div>

    <Dialog.Footer class="mt-4">
      <Button onclick={() => (open = false)} variant="outline">Close</Button>
    </Dialog.Footer>
  </Dialog.Content>
</Dialog.Root>


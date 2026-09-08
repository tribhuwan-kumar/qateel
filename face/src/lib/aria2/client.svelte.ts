import { toast } from "svelte-sonner";
import { writable } from 'svelte/store';
import { secureFetch } from "$lib/utils";
import { browser } from "$app/environment";
import { truncateMiddle } from "$lib/utils";
import { selectedGids } from "$lib/selection";
import type {
  StopReq,
  PauseReq,
	TickTask,
  ResumeReq,
	AddUriReq,
	UriInfoRes,
	UriInfoReq,
  GlobalStat,
	AddUriTask,
  DdlWsMessage,
  Aria2Options,
  ItemMetaData,
	SearchParams,
	SearchResponse,
  ChangeOptionReq,
  HistoryResponse,
	Aria2CmdOptions,
  Aria2GlobalOptions,
	ChangeCmdOptionReq,
  ChangeGlobalOptionReq,
} from "$lib/aria2/types";
import {
	SvelteURL,
	SvelteSet,
	SvelteMap,
	SvelteDate,
	SvelteURLSearchParams
} from "svelte/reactivity";

export const UNTITLED: string = "<Untitled>";

export const defaultGlobalStat: GlobalStat = {
  downloadSpeed: "0",
  uploadSpeed: "0",
  numActive: "0",
  numStopped: "0",
  numStoppedTotal: "0",
  numWaiting: "0",
};

export const aria2Store = $state({
  items: [] as ItemMetaData[],
	global: { ...defaultGlobalStat } as GlobalStat,
  connection: "disconnected" as "disconnected" | "connecting" | "connected",
});

export function setDefaultStat() {
  aria2Store.global = { ...defaultGlobalStat };
}

/// Proably a json
export const aria2Info = writable<Record<string, any>>();
export const itemCounts = writable<Record<string, any>>();

export class Aria2Manager {
  private ddlWs: WebSocket | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;

  private globalStatWs: WebSocket | null = null;
  private globalStatRetryTimer: ReturnType<typeof setTimeout> | null = null;
  private globalStatWatchdog: ReturnType<typeof setTimeout> | null = null;

  constructor() {}

  private resetWatchdog() {
    if (this.globalStatWatchdog) clearTimeout(this.globalStatWatchdog);
    this.globalStatWatchdog = setTimeout(() => {
      setDefaultStat();
    }, 5000);
  }

  connectDdlWs() {
    if (!browser || this.ddlWs) return;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const host = window.location.host;
    const url = `${protocol}//${host}/api/ws/ddl`;

    aria2Store.connection = "connecting";
    this.ddlWs = new WebSocket(url);

    this.ddlWs.onopen = () => {
      console.log("Connected to tick stream");
      aria2Store.connection = "connected";
    };

    this.ddlWs.onclose = () => {
      console.log("Disconnected from tick stream");
      aria2Store.connection = "disconnected";
      this.ddlWs = null;
      if (this.retryTimer) clearTimeout(this.retryTimer);
      this.retryTimer = setTimeout(() => this.connectDdlWs(), 5000);
    };

    this.ddlWs.onmessage = (event) => this.handleDdlMessage(event);
  }

  private handleDdlMessage(event: MessageEvent) {
    try {
      const msg: DdlWsMessage = JSON.parse(event.data);

      if (msg.type === "tick") {
        // Use `SvelteMap` and `SvelteSet`
				const seen = new SvelteSet<string>();
        const taskByGid = new SvelteMap<string, TickTask>(msg.tasks.map(task => [task.gid, task]));

        // Update existing items in place
        for (let i = 0; i < aria2Store.items.length; i++) {
          const item = aria2Store.items[i];
          const task = taskByGid.get(item.gid);

          if (task) {
            seen.add(item.gid);

            // Volatile data: overwrite unconditionally
            item.status = task.status;
            item.totalLength = task.totalLength;
            item.completedLength = task.completedLength;
            item.uploadLength = task.uploadLength;
            item.downloadSpeed = task.downloadSpeed;
            item.uploadSpeed = task.uploadSpeed;
            item.connections = task.connections;
            item.verifiedLength = task.verifiedLength;
            item.dir = task.dir;
            // Static data merge; fallbacks
            if (task.name && task.name !== UNTITLED) {
              item.name = task.name;
            } else if (task.name && !item.name) {
              item.name = task.name;
            }
            if (task.kind) item.kind = task.kind;
            if (task.sourceUri) item.sourceUri = task.sourceUri;
            if (task.verifyIntegrityPending !== null && task.verifyIntegrityPending !== undefined) {
              item.verifyIntegrityPending = task.verifyIntegrityPending;
            }
            if (task.num_seeders) item.numSeeder = task.num_seeders;
          }
        }

        // Add new unseen tasks
        const newItems = msg.tasks
          .filter(task => !seen.has(task.gid))
          .map(task => ({
            gid: task.gid,
            name: task.name,
            status: task.status,
            dir: task.dir,
            kind: task.kind || null,
            files: "[]",
            totalLength: task.totalLength,
            completedLength: task.completedLength,
            uploadLength: task.uploadLength,
            contentHash: null,
            sourceUri: task.sourceUri || null,
            infoHash: null,
            seeder: null,
            errorCode: null,
            errorMessage: null,
            torrent: null,
            options: null,
            isResumeSupported: null, // Unknown right now. Event websocket will fill this in!
            connections: task.connections,
            numPieces: null,
            numSeeder: task.num_seeders || null,
            pieceLength: null,
            verifiedLength: task.verifiedLength,
            verifyIntegrityPending: task.verifyIntegrityPending,
            createdAt: new SvelteDate().toISOString(),
            completedAt: null,
            downloadSpeed: task.downloadSpeed,
            uploadSpeed: task.uploadSpeed,
          } as ItemMetaData));

        if (newItems.length > 0) {
          aria2Store.items.unshift(...newItems);
        }

      } else if (msg.type === "event") {
        this.handleDdlEvent(msg.data);
      }
    } catch (e) {
      console.error("WS error", e);
    }
  }

  private handleDdlEvent(newItem: ItemMetaData) {
    const index = aria2Store.items.findIndex(i => i.gid === newItem.gid);
    if (index !== -1) {
      // Object.assign merges the new DB truth, but preserve the volatile speeds!
      const currentDownloadSpeed = aria2Store.items[index].downloadSpeed;
      const currentUploadSpeed = aria2Store.items[index].uploadSpeed;

      Object.assign(aria2Store.items[index], newItem);

      // Re-apply speeds since `newItem` (DB) doesn't have them
      if (currentDownloadSpeed) aria2Store.items[index].downloadSpeed = currentDownloadSpeed;
      if (currentUploadSpeed) aria2Store.items[index].uploadSpeed = currentUploadSpeed;
    } else {
      aria2Store.items.unshift(newItem);
    }
  }

  connectGlobalStatWs() {
    if (!browser || this.globalStatWs) return;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const host = window.location.host;
    const globalStatWsUrl = `${protocol}//${host}/api/ws/global/stat`;

    aria2Store.connection = "connecting";
    this.globalStatWs = new WebSocket(globalStatWsUrl);

    this.globalStatWs.onopen = () => {
      console.log("Connected to global stat stream");
      aria2Store.connection = "connected";
      this.resetWatchdog();
    };

    this.globalStatWs.onmessage = (event) => {
      this.resetWatchdog();
      try {
        const msg = JSON.parse(event.data);
        if (msg.type === "global" && msg.data) {
          aria2Store.global = msg.data;
        }
      } catch (e) {
        console.error("Bad message", e);
      }
    };

    this.globalStatWs.onclose = () => {
      console.warn("Global stat stream disconnected");
      aria2Store.connection = "disconnected";
      this.globalStatWs = null;
      setDefaultStat();
      if (this.globalStatRetryTimer) clearTimeout(this.globalStatRetryTimer);
      this.globalStatRetryTimer = setTimeout(() => this.connectGlobalStatWs(), 5000);
    };

    this.globalStatWs.onerror = () => {
      this.globalStatWs?.close();
    };
  }

  async loadInitialData(page = 1) {
    try {
      const res = await secureFetch(`/api/qateel/user/history?page=${page}&limit=15`);
      const json: HistoryResponse = await res.json();

      if (json.data) {
        if (page === 1) {
          aria2Store.items = json.data;
        } else {
          const seen = new SvelteSet(aria2Store.items.map(item => item.gid));
          for (const item of json.data) {
            if (!seen.has(item.gid)) {
              aria2Store.items.push(item);
            }
          }
        }
      }
      return json;
    } catch (e) {
      console.error(e);
      return null;
    }
  }

  async delete(gids: string[], deleteFile: boolean) {
    try {
      const res = await secureFetch("/api/qateel/gid/delete", {
        method: "DELETE",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({
					gids: gids,
					deleteFile: deleteFile
				})
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || "Deletion failed");
      }

			const data = await res.json().catch(() => null);
			if (data.status === "deleted") {
				aria2Store.items = aria2Store.items.filter(i => !gids.includes(i.gid));
				selectedGids.set([]);
			}
			console.log("Data from deletion endpoint:", data)
			await this.getItemsCount();
      toast.success(`${gids.length === 1 ?
							"Successfully deleted!!" : `Deleted ${gids.length} items!!`}`, {
        richColors: true,
        style: "cursor: pointer;"
      });

    } catch (e) {
      const message = this.extractErrorMessage(e)
      toast.error("Deletion failed:", {
				description: message,
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }

  /**
   * Retries a download using its GID.
   * If "action" is omitted, the server checks for duplicates.
   * If a duplicate is found, it returns a conflict object.
  */
  // async retry(gid: string): Promise<RetryResult> {
  //   const payload: RetryReq = { gid, action };
  //   try {
  //     const res = await secureFetch("/api/qateel/gid/retry", {
  //       method: "POST",
  //       headers: { "Content-Type": "application/json" },
  //       body: JSON.stringify(payload)
  //     });
  //
  //     if (res.status === 409) {
  //       const conflict: DuplicateConflictError = await res.json();
  //       return { success: false, conflict };
  //     }
  //
  //     if (!res.ok) {
  //       const err = await res.json();
  //       throw new Error(err.error || "Retry failed");
  //     }
  //
  //     const data = await res.json();
  //     toast.success(`${action === "overwrite" ? "Restarted (Overwriting)" : "Retrying download..."}`, {
  //       richColors: true,
  //       style: "cursor: pointer;"
  //     });
  //
  //     return { success: true, newGid: data.newGid };
  //
  //   } catch (e: any) {
  //     console.error(e);
  //     toast.error(e.message || "Failed to retry");
  //     return { success: false, error: e.message };
  //   }
  // }

	/**
		* Submits finalized tasks to Aria2 after the user
		* has reviewed the preflight data.
   */
	async addUris(payload: AddUriReq): Promise<any> {
    if (!payload || payload.tasks.length === 0) return null;
    try {
      const res = await secureFetch(`/api/qateel/add/uris`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });

      const resAsJson = await res.json().catch(() => null);

      if (!res.ok) {
        const message = resAsJson?.error
					|| resAsJson?.message
					|| `Failed to add ${payload.tasks.length === 1 ? "URI" : `${payload.tasks.length} URIs`}`;
        toast.error(message, { richColors: true, style: "cursor: pointer;" });
        throw new Error(message);
      }

      if (!resAsJson || !Array.isArray(resAsJson.data)) {
        const message = "Server returned an invalid response while adding URIs";
        toast.error(message, {
					richColors: true,
					style: "cursor: pointer;"
				});
        throw new Error(message);
      }

      resAsJson.data.forEach((result: any, fallbackIndex: number) => {
        const index = typeof result.index === "number" ? result.index : fallbackIndex;
        const task = payload.tasks[index];
        const uri = task?.name ?? `URI: ${index + 1}`;

        if (result?.error) {
          const message: string = result?.error || "Something went wrong";
          toast.error(`Failed to add URI  :(`, {
            description: `${message.replace(/\.$/, "").replace(/--/g, "")}`,
            richColors: true,
            style: "cursor: pointer;"
          });
        } else {
          toast.success(`Added to download  :)`, {
            description: `Name: ${truncateMiddle(uri, 40)}`,
            richColors: true,
            style: "cursor: pointer;"
          });
					this.getItemsCount();
        }
      });

      return resAsJson.data;
    } catch (e) {
      console.error("Error adding URIs:", e);
      throw e;
    }
  }

	/**
   * Probes URIs asynchronously without blocking.
   * Use this to fetch metadata and duplication state before showing the final "Add" dialog.
   */
  async probeUris(uris: string[]): Promise<UriInfoRes[] | null> {
    if (!uris || uris.length === 0) return [];
    try {
      const payload: UriInfoReq = { uris };
      const res = await secureFetch(`/api/qateel/probe/uris`, {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });
      if (!res.ok) {
        console.error("Failed to probe URIs:", await res.text());
				toast.error("Failed to probe URIs:", {
					description: await res.text(),
					richColors: true,
					style: "cursor: pointer;"
				});
        return null;
      }
      const json = await res.json();
      return json.data as UriInfoRes[];
    } catch (e) {
      console.error("Network error while probing URIs:", e);
      const message = this.extractErrorMessage(e);
      toast.error("Failed to probe URIs:", {
        description: message,
        richColors: true,
        style: "cursor: pointer;"
      });
      return null;
    }
  }

	/**
   * Checks if a specific file name already exists on the server.
   */
  async checkName(name: string): Promise<{
		isConflict: boolean;
		msg?: string
	}> {
    if (!name || name.trim() === "") return { isConflict: false };
    try {
      const res = await secureFetch(`/api/qateel/file/conflict`, {
        method: "POST",
        headers: {
					"Content-Type": "application/json"
				},
        body: JSON.stringify({ name })
      });
      if (!res.ok) return {
				isConflict: false
			};
      return await res.json();
    } catch {
      return { isConflict: false };
    }
  }

  /*
   * Adds multiple torrent as batch to the queue
   * Multicall
  */
  async addTorrents(torrents: { torrent: string, options?: any }[]) {
    const res = await secureFetch("/api/qateel/add/torrents", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ torrents })
    });

    if (!res.ok) {
      const err = await res.json();
      console.error("Failed to add torrents:", err);
      throw new Error(err.error || "Failed to add torrents");
    }

    toast.success(`Added ${torrents.length === 1 ? "torrent" : `${torrents.length} torrents`} to download`, {
      richColors: true,
      style: "cursor: pointer;"
    });
		await this.getItemsCount();
    return await res.json();
  }


  /**
   * Pause a specific download.
   * @param gid The GID of the download to pause.
   * @param force If true, uses "aria2.forcePause" (immediate).
   */
	async pause(gids: string[], force = false) {
    const payload: PauseReq = {
      gids: gids,
      forcePause: force
    };
    await this.sendPauseRequest(payload);
  }


  /**
   * Pause ALL active downloads.
   * @param force If true, uses "aria2.forcePauseAll".
   */
  async pauseAll(force = false) {
    const payload: PauseReq = {
      pauseAll: !force,
      forcePauseAll: force
    };
    await this.sendPauseRequest(payload);
  }


  /**
   * Internal helper to send the pause command.
   */
	private async sendPauseRequest(payload: PauseReq) {
    try {
      const res = await secureFetch("/api/qateel/gid/pause", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || "Pause action failed");
      }

      // Determine the toast message context
      let action = payload.forcePause || payload.forcePauseAll ? "force paused" : "paused";
      if (payload.pauseAll || payload.forcePauseAll) {
        action += " all downloads";
      } else if (payload.gids && payload.gids.length > 1) {
        action += ` ${payload.gids.length} downloads`;
      } else {
        action += " download";
      }

      toast.success(`Successfully ${action}!!`, {
        richColors: true,
        style: "cursor: pointer;"
      });

    } catch (e) {
      console.error(e);
      const message = this.extractErrorMessage(e);
      toast.error("Failed to pause:", {
        description: message,
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }


  /**
   * Resume a specific download (unpause).
   * @param gids The GID of the download to resume.
   */
	async resume(gids: string[]) {
    const payload: ResumeReq = {
      gid: gids
    };
    await this.sendResumeRequest(payload);
  }


  /**
   * Resume all paused downloads (unpauseAll).
  */
	async resumeAll() {
    const payload: ResumeReq = { resumeAll: true };
    await this.sendResumeRequest(payload);
  }


  /**
   * Internal helper to send the resume command.
  */
	private async sendResumeRequest(payload: ResumeReq) {
    try {
      const res = await secureFetch("/api/qateel/gid/resume", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || "Resume failed");
      }

      let action = "resumed download";
      if (payload.resumeAll) {
        action = "resumed all downloads";
      } else if (payload.gid && payload.gid.length > 1) {
        action = `resumed ${payload.gid.length} downloads`;
      }

      toast.success(`Successfully ${action}!!`, {
        richColors: true,
        style: "cursor: pointer;"
      });

    } catch (e) {
      console.error(e);
      const message = this.extractErrorMessage(e);
      toast.error("Failed to resume:", {
        description: message,
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }


  /**
   * Stop a specific download
   * @param gids The GIDs of the download to stop.
   */
	async stop(gids: string[], force = false) {
    const payload: StopReq = {
      gids: gids,
      forceStop: force
    };
    await this.sendStopRequest(payload);
  }


  /**
   * Stop all paused downloads
   */
	async forceStop(gids: string[]) {
    await this.stop(gids, true);
	}


  /**
   * Internal helper to send the stop command.
   */
  private async sendStopRequest(payload: StopReq) {
    try {
      const res = await secureFetch("/api/qateel/gid/stop", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });

      const resAsJson = await res.json().catch(() => null);

      if (!res.ok) {
        const message = resAsJson?.error
					|| resAsJson?.message
					|| `Failed to add ${payload.gids?.length === 1 ? "URI" : `${payload.gids?.length} URIs`}`;
        toast.error(message, { richColors: true, style: "cursor: pointer;" });
        throw new Error(resAsJson);
      }

      const isForce = payload.forceStop;
      const count = payload.gids?.length || 0;
      const action = isForce ? "Forcefully stopped" : "Stopped";
      const itemLabel = count > 1 ? `${count} downloads` : "download";

      toast.success(`${action} ${itemLabel}!!`, {
        richColors: true,
        style: "cursor: pointer;"
      });

    } catch (e: any) {
			console.log("Stop error:", e);
			let errorData: any;
			try {
				errorData = JSON.parse(e) || "{}";
				console.log("errorData:", errorData);
			} catch {
				errorData = null;
			}
			const failedGids = errorData?.failed
				? errorData.failed.map((item: any) => item.gid).join(", ")
				: "Unknown error";

      toast.error(`${failedGids}:`, {
				description: failedGids,
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }


  /**
   * Fetches the current global configuration from Aria2.
   */
  async getOption(gid: string): Promise<Aria2Options> {
    try {
      const res = await secureFetch("/api/qateel/gid/get/options", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify({ gid })
      });
      if (!res.ok) {
        throw new Error("Failed to fetch options");
      }

      const data = await res.json();

      return  data.data
    } catch (e: any) {
      console.error(e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to load current settings"}`, {
        closeButton: true,
        richColors: true,
      });
      return {};
    }
  }

  /**
   * Dynamically changes options of a specific download.
   * NOTE: For active downloads, most options will cause aria2 to automatically
   * restart the download to apply them (except limits, peers, etc).
  */
  async changeOption(gid: string, options: Aria2Options) {
    const payload: ChangeOptionReq = { gid, options };

    try {
      const res = await secureFetch("/api/qateel/gid/set/options", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || "Failed to change options");
      }

      toast.success("Options updated successfully", {
        closeButton: true,
        richColors: true,
      });

    } catch (e: any) {
      console.error(e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to change options"}`, {
        closeButton: true,
        richColors: true,
      });
    }
  }


  /**
   * Fetches the current global configuration from Aria2.
   */
  async getGlobalOption(): Promise<Aria2GlobalOptions> {
    try {
      const res = await secureFetch("/api/qateel/global/get/options");
      const data = await res.json();
      if (!res.ok) {
        const err = data;
        throw new Error(err.error || "Failed to fetch global options");
      }
      return  data.data
    } catch (e: any) {
      console.error(e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to load global settings"}`, {
        closeButton: true,
        richColors: true,
      });
      return {};
    }
  }

  /**
   * Fetches the current Aria2 info
   */
  async getAria2Info(): Promise<any> {
    try {
      const res = await secureFetch("/api/qateel/info");
      const results = await res.json();
      if (!res.ok) {
        const err = results;
        throw new Error(err.error || "Failed to fetch aria2 info!!");
      }
      return results.data
    } catch (e: any) {
      console.error(e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to load aria2 info"}`, {
        closeButton: true,
        richColors: true,
      });
      return {};
    }
  }

  /**
   * Fetches the items counts based on category
   */
	// don't wanna map all the static type, keeping it `any`
  async getItemsCount(): Promise<void> {
    try {
      const res = await secureFetch("/api/qateel/user/items/count");
      const results = await res.json();
      if (!res.ok) {
        const err = results;
        throw new Error(err.error || "Failed to fetch item counts.");
      }
			itemCounts.set(results.data);
    } catch (e: any) {
      console.error(e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to fetch item counts."}`, {
        closeButton: true,
        richColors: true,
      });
    }
  }

  /**
   * Dynamically changes global options.
   * These settings will act as defaults for all newly added downloads.
   * Limits apply instantly to all active downloads.
   */
  async changeGlobalOption(options: Aria2GlobalOptions) {
    const payload: ChangeGlobalOptionReq = { options };

    try {
      const res = await secureFetch("/api/qateel/global/set/options", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });

      const data = await res.json();
      if (!res.ok) {
        const err = data;
        throw new Error(err.error || "Failed to change global options");
      }

      toast.success("Global settings updated!!", {
        richColors: true,
				style: "cursor: pointer;"
      });

    } catch (e: any) {
      console.log(e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to save global settings"}`, {
        closeButton: true,
        richColors: true,
      });
    }
  }

  async changeCmdOption(options: Aria2CmdOptions) {
    const payload: ChangeCmdOptionReq = { options };

    try {
      const res = await secureFetch("/api/qateel/cmd/set/options", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
        body: JSON.stringify(payload)
      });

      const data = await res.json();
      if (!res.ok) {
        const err = data;
        throw new Error(err.error || "Failed to set cmd options");
      }

      toast.success("Global options updated!!", {
				description: "Daemon will restart now.",
        richColors: true,
				style: "cursor: pointer;"
      });

    } catch (e: any) {
      console.log(e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to save cmd settings"}`, {
        closeButton: true,
        richColors: true,
      });
    }
  }

	/**
		* Perform search using server endpoint.
		* Returns paginated and ranked results from the server.
		*/
	async search(params: SearchParams): Promise<SearchResponse> {
		try {
			const url = new SvelteURL("/api/qateel/search", window.location.origin);
			const sp = new SvelteURLSearchParams();

			// All in camelcase
			if (params.q) sp.set("q", params.q);
			if (params.fileType) sp.set("fileType", params.fileType);
			if (params.extension) sp.set("extension", params.extension);
			if (params.page) sp.set("page", String(params.page));
			if (params.perPage) sp.set("perPage", String(params.perPage));
			url.search = sp.toString();

			const res = await secureFetch(url.pathname + (url.search ? `?${url.searchParams.toString()}` : ""));
			if (!res.ok) {
				const err = await res.json().catch(() => null);
				throw new Error(err?.error || "Search request failed");
			}
			const json = await res.json();
			console.log("Search res:", json);

			return {
				data: json.data ?? [],
				meta: {
					page: json.page ?? 1,
					perPage: json.perPage ?? 20,
					total: json.total ?? 0,
				}
			};
		} catch (e) {
			console.error("Search failed:", e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to search!!"}`, {
        closeButton: true,
        richColors: true,
      });
		}
	}






























  private extractErrorMessage = (value: unknown): string => {
    if (value instanceof Error) {
      return this.extractErrorMessage(value.message);
    }
    if (typeof value === "string") {
      try {
        const parsed = JSON.parse(value);
        if (typeof parsed === "string") {
          return parsed;
        }
        if (parsed && typeof parsed === "object") {
          const obj = parsed as Record<string, unknown>;
          if (typeof obj.message === "string") {
            return obj.message;
          }
          if (typeof obj.error === "string") {
            return this.extractErrorMessage(obj.error);
          }
        }
      } catch {
        // not JSON, use as is
      }

      return value;
    }
    return "Failed to extract error message";
  };
}

export const aria2 = new Aria2Manager();


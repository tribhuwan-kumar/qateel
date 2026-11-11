import { get } from 'svelte/store';
import { toast } from "svelte-sonner";
import { writable } from 'svelte/store';
import { historySort } from "$lib/sort";
import { secureFetch } from "$lib/utils";
import { browser } from "$app/environment";
import { truncateMiddle } from "$lib/utils";
import { type Order, type SortBy } from "$lib/sort";
import { selectedGids, isActivelySelecting } from "$lib/selection";
import type {
	Kind,
  StopReq,
  PauseReq,
	TickTask,
  ResumeReq,
	AddUriReq,
	GidStatus,
	UriInfoRes,
	UriInfoReq,
  GlobalStat,
	SpeedRecord,
  DlWsMessage,
  Aria2Options,
  ItemMetaData,
	SearchParams,
	SearchResponse,
  ChangeOptionReq,
  HistoryResponse,
	Aria2CmdOptions,
	HistoryQueryParams,
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
import {
	selectedStatus,
	selectedCategory,
	getItemCategory
} from "$lib/categorize";

let TOTAL_ITEMS = $state<number>(0);

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
  private dlWs: WebSocket | null = null;
  private retryTimer: ReturnType<typeof setTimeout> | null = null;

  private globalStatWs: WebSocket | null = null;
  private globalStatRetryTimer: ReturnType<typeof setTimeout> | null = null;
  private globalStatWatchdog: ReturnType<typeof setTimeout> | null = null;

	// Buffer insertions while drag selection is active so rows don't shift under the pointer
	private pendingNewItems: ItemMetaData[] = [];

	constructor() {
		if (browser) {
			isActivelySelecting.subscribe((selecting) => {
				if (!selecting && this.pendingNewItems.length > 0) {
					const sort = get(historySort);
					for (const item of this.pendingNewItems) {
						this.insertSortedItem(aria2Store.items, item, sort?.sortBy ?? "time", sort?.order ?? "desc");
					}
					this.pendingNewItems = [];
				}
			});
		}
	}

  private resetWatchdog() {
    if (this.globalStatWatchdog) clearTimeout(this.globalStatWatchdog);
    this.globalStatWatchdog = setTimeout(() => {
      setDefaultStat();
    }, 5000);
  }

  connectDlWs() {
    if (!browser || this.dlWs) return;

    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    const host = window.location.host;
    const url = `${protocol}//${host}/api/ws/dl`;

    aria2Store.connection = "connecting";
    this.dlWs = new WebSocket(url);

    this.dlWs.onopen = () => {
      console.log("Connected to dl ws.");
      aria2Store.connection = "connected";
    };

    this.dlWs.onclose = () => {
      console.log("Disconnected from tick stream");
      aria2Store.connection = "disconnected";
      this.dlWs = null;
      if (this.retryTimer) clearTimeout(this.retryTimer);
      this.retryTimer = setTimeout(() => this.connectDlWs(), 5000);
    };

    this.dlWs.onmessage = (event) => this.handleDlMessage(event);
  }

	private async handleDlMessage(event: MessageEvent) {
		try {
			const msg: DlWsMessage = JSON.parse(event.data);
			if (msg.type === "tick") {
				const existingGids = new SvelteSet(aria2Store.items.map(i => i.gid));
				const taskByGid = new SvelteMap<string, TickTask>(msg.tasks.map(task => [task.gid, task]));

				for (let i = 0; i < aria2Store.items.length; i++) {
					const item = aria2Store.items[i];
					const task = taskByGid.get(item.gid);
					if (!task) continue;

					// Live telemetry
					item.status = task.status;
					item.totalLength = task.totalLength;
					item.completedLength = task.completedLength;
					item.uploadLength = task.uploadLength;
					item.downloadSpeed = task.downloadSpeed;
					item.uploadSpeed = task.uploadSpeed;

					// Nullable merge
					if (task.connections != null) item.connections = task.connections;
					if (task.verifiedLength != null) item.verifiedLength = task.verifiedLength;
					if (task.dir != null) item.dir = task.dir;
					if (task.files != null) item.files = task.files;
					if (task.sourceUri != null) item.sourceUri = task.sourceUri;
					if (task.verifyIntegrityPending != null) item.verifyIntegrityPending = task.verifyIntegrityPending;
					if (task.isResumeSupported != null) item.isResumeSupported = task.isResumeSupported;
					if (task.numSeeders != null) item.numSeeders = task.numSeeders;
					if (task.seeder != null) item.seeder = task.seeder;
					if (task.kind) item.kind = task.kind;

					if (task.name && task.name !== UNTITLED) {
						item.name = task.name;
					} else if (task.name && !item.name) {
						item.name = task.name;
					}
				}

				// New tasks that match current active filter and sort
				const sort = get(historySort);
				const isSelecting = get(isActivelySelecting);
				for (const task of msg.tasks) {
					if (!existingGids.has(task.gid)) {
						if (this.matchesCurrentFilter(task)) {
							const newItem = this.tickTaskToItemMetaData(task);
							existingGids.add(task.gid);

							if (isSelecting) {
								if (!this.pendingNewItems.some(p => p.gid === newItem.gid)) {
									this.pendingNewItems.push(newItem);
								}
							} else {
								this.insertSortedItem(aria2Store.items, newItem, sort?.sortBy ?? "time", sort?.order ?? "desc");
							}
						}
					}
				}

			} else if (msg.type === "event") {
				console.log("Event message:", msg.data);
				for (const item of msg.data){
					this.handleDlEvent(item);
				}
			}
		} catch (e) {
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "WebSocket error!!"}`, {
        closeButton: true,
        richColors: true,
      });
			console.error("WS error:", e);
		} finally {
			await this.getItemsCount()
		}
	}

	private handleDlEvent(newItem: ItemMetaData) {
		const index = aria2Store.items.findIndex(i => i.gid === newItem.gid);
		const isStreaming = (s: GidStatus) => s === "active" || s === "seeding";
		console.log("Event item index:", index);
		if (index !== -1) {
			const currentDownloadSpeed = aria2Store.items[index].downloadSpeed ?? "0";
			const currentUploadSpeed = aria2Store.items[index].uploadSpeed ?? "0";
			const currentNumSeeders = aria2Store.items[index].numSeeders ?? "0";
			const currentconnections = aria2Store.items[index].connections ?? "0";

			Object.assign(aria2Store.items[index], newItem);

			// preserve speed only for streaming states
			if (isStreaming(aria2Store.items[index].status)) {
				aria2Store.items[index].downloadSpeed = currentDownloadSpeed;
				aria2Store.items[index].uploadSpeed = currentUploadSpeed;
				aria2Store.items[index].numSeeders = currentNumSeeders;
				aria2Store.items[index].connections = currentconnections;
			} else {
				aria2Store.items[index].downloadSpeed = "0";
				aria2Store.items[index].uploadSpeed = "0";
				aria2Store.items[index].numSeeders = "0";
				aria2Store.items[index].connections = "0";
			}

			// If filter changed such that it no longer matches the current filter, remove it cleanly
			if (!this.matchesCurrentFilter(aria2Store.items[index])) {
				aria2Store.items.splice(index, 1);
			}

		} else {
			if (this.matchesCurrentFilter(newItem)) {
				if (!isStreaming(newItem.status)) {
					newItem.downloadSpeed = "0";
					newItem.uploadSpeed = "0";
					newItem.connections = "0";
					newItem.numSeeders = "0";
				}
				const sort = get(historySort);
				const isSelecting = get(isActivelySelecting);

				if (isSelecting) {
					if (!this.pendingNewItems.some(p => p.gid === newItem.gid)) {
						this.pendingNewItems.push(newItem);
					}
				} else {
					this.insertSortedItem(aria2Store.items, newItem, sort?.sortBy ?? "time", sort?.order ?? "desc");
				}
			}
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
      console.log("Connected to global stat ws.");
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
        console.error("Bad message:", e);
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

	/**
	 * Fetches paginated download history filtered by status and category.
	 */
	async getHistory(params: HistoryQueryParams = {}): Promise<HistoryResponse | null> {
		try {
			const page = params.page ?? 1;
			const limit = params.limit ?? 15;

			const sp = new SvelteURLSearchParams();
			sp.set("page", String(page));
			sp.set("limit", String(limit));

			if (params.sortBy) sp.set("sortBy", params.sortBy);
			if (params.order) sp.set("order", params.order);
			if (params.status && params.status !== "all") sp.set("status", params.status);
			if (params.category && params.category !== "everything") sp.set("category", params.category);

			console.log(`Get history params: '${sp.toString()}'`)
			const res = await secureFetch(`/api/qateel/user/history?${sp.toString()}`);

			if (!res.ok) {
				const errorData = await res.json();
				throw new Error(JSON.stringify(errorData));
			}

			const json: HistoryResponse = await res.json();

			if (json.data) {
				if (page === 1) {
					aria2Store.items = json.data;
				} else {
					const seen = new SvelteSet(aria2Store.items.map((item) => item.gid));
					for (const item of json.data) {
						if (!seen.has(item.gid)) {
							 aria2Store.items.push(item);
						}
					}
				}
			}

			return json;
		} catch (e) {
			console.error("Error in getHistory:", e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to get history :("}`, {
        closeButton: true,
        richColors: true,
      });
			return null;
		}
	}

	/// This is so inefficient
	async getAllHistory(status?: string, category?: string): Promise<ItemMetaData[]> {
		try {
			await this.getItemsCount();
			const sp = new SvelteURLSearchParams();

			sp.set("page", "1");
			sp.set("limit", String(TOTAL_ITEMS));											// Make the `TOTAL_ITEMS` limit when select all

			if (status && status !== "all") sp.set("status", status);
			if (category && category !== "everything") sp.set("category", category);

			console.log(`Get all history params: '${sp.toString()}'`)
			const res = await secureFetch(`/api/qateel/user/history?${sp.toString()}`);
			if (!res.ok) return [];
			const json: HistoryResponse = await res.json();

			return json.data ?? [];
		} catch (e) {
			console.error("Error fetching all history:", e);
      const message = this.extractErrorMessage(e)
      toast.error(`${message ? message : "Failed to get history :("}`, {
        closeButton: true,
        richColors: true,
      });
			return [];
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
			console.log("Deleting:", data)

			await this.getItemsCount();

      toast.success(`${gids.length === 1 ? "Successfully deleted!!" : `Deleted ${gids.length} items!!`}`, {
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
		* return stuff just see the aria2 proxy
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

				if (payload.tasks.length < 5) {
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
					}
				}
      });

			if (payload.tasks.length > 5) {
				toast.success(`${payload.tasks.length} URIs added to download  :)`, {
					richColors: true,
					style: "cursor: pointer;"
				});
			}

			await this.getItemsCount();

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
        throw new Error(err.error || "Pause action failed.");
      }

      const data = await res.json();
			/*
				On success
				json!({
					"status": "paused",
					"method": method,
					"succeeded": succeeded,				vec string
					"failed": failed							vec value
				})
			*/
      // Determine the toast message context
      let action = payload.forcePause ? "force paused" : "paused";
      if (
				payload.gids
				&& payload.gids.length > 1
				&& data?.status === "paused"
				&& data?.succeeded.length > 1
			) {
        action += ` ${data?.succeeded.length} downloads`;
      } else if (
				payload.gids
				&& payload.gids.length === 1
				&& data?.status === "paused"
				&& data?.succeeded?.length === 1
			) {
        action += " download";
      }

			if (
				payload.gids
				&& payload.gids.length >= 1
				&& data?.status === "paused"
				&& data?.failed.length >= 1
			) {
				toast.error(`Failed to ${payload.forcePause ? "force pause" : "pause"} ${data?.failed.length} download!!`, {
					richColors: true,
					style: "cursor: pointer;"
				});
      } else {
				toast.success(`Successfully ${action}!!`, {
					richColors: true,
					style: "cursor: pointer;"
				});
			}

    } catch (e) {
      console.error(e);
      const message = this.extractErrorMessage(e);
      toast.error(`${message ? message : "Failed to pause :("}`, {
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
      gids: gids
    };
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
        throw new Error(err.error || "Resume failed.");
      }

      const data = await res.json();
			/*
				On success
				json!({
					"status": "resumed",
					"method": method,
					"succeeded": succeeded,				vec string
					"failed": failed							vec value
				})
			*/
      let action = "resumed";
      if(
				payload.gids
				&& payload.gids.length === 1
				&& data?.status === "resumed"
				&& data?.succeeded?.length === 1
			) {
        action = `resumed download`;
			} else if (
				payload.gids
				&& payload.gids.length > 1
				&& data?.status === "resumed"
				&& data?.succeeded.length > 1
			) {
        action = `resumed ${data?.succeeded.length} downloads`;
      }

			if (
				payload.gids
				&& payload.gids.length >= 1
				&& data?.status === "resumed"
				&& data?.failed.length >= 1
			) {
				toast.error(`Failed to resume ${data?.failed.length} download!!`, {
					richColors: true,
					style: "cursor: pointer;"
				});
      } else {
				toast.success(`Successfully ${action}!!`, {
					richColors: true,
					style: "cursor: pointer;"
				});
			}
    } catch (e) {
      console.error(e);
      const message = this.extractErrorMessage(e);
      toast.error(`${message ? message : "Failed to resume :("}`, {
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
			/*
				On success
				json!({
					"status": "stopped",
					"method": method,
					"succeeded": succeeded,				vec string
					"failed": failed							vec value
				})
			*/
      if (!res.ok) {
        const message = resAsJson?.error
					|| `Failed to stop ${payload.gids?.length === 1 ? "URI" : `${payload.gids?.length} URIs`}`;
        toast.error(message, {
					richColors: true,
					style: "cursor: pointer;"
				});
        throw new Error(resAsJson);
      }

      const isForce = payload.forceStop;
      const count = resAsJson?.succeeded.length || 0;
      const action = isForce ? "Forcefully stopped" : "Stopped";
      const itemLabel = count > 1 ? `${count} downloads` : "download";

      toast.success(`${action} ${itemLabel}!!`, {
        richColors: true,
        style: "cursor: pointer;"
      });

			if (
				payload.gids
				&& payload.gids.length >= 1
				&& resAsJson?.status === "stopped"
				&& resAsJson?.failed.length >= 1
			) {
				toast.error(`Failed to ${payload.forceStop ? "force stop" : "stop"} ${resAsJson?.failed.length} download!!`, {
					richColors: true,
					style: "cursor: pointer;"
				});
      }

    } catch (e: any) {
      console.error(e);
      const message = this.extractErrorMessage(e);
      toast.error(`${message ? message : "Failed to :("}`, {
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }

	async stopAll() {
    try {
      const res = await secureFetch("/api/qateel/gid/stop/all", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || "Stop all action failed.");
      }

      const data = await res.json();
			/*
				On success
			json!({
				"status": "stoppedAll",
				"succeeded": successful_gids,
				"failed": failed
      })
			*/
			let action = "stopped all";
      if (data?.status === "stoppedAll" && data?.succeeded.length > 1) {
        action += ` ${data?.succeeded.length} downloads`;
      } else if (data?.status === "stoppedAll" && data?.succeeded?.length === 1) {
        action += " download";
      }

			toast.success(`Successfully ${action}!!`, {
				richColors: true,
				style: "cursor: pointer;"
			});

			if (data?.status === "stoppedAll" && data?.failed.length >= 1) {
				toast.error(`Failed to stop all ${data?.failed.length} download!!`, {
					richColors: true,
					style: "cursor: pointer;"
				});
      }

			await this.getItemsCount()

    } catch (e) {
      console.error(e);
      const message = this.extractErrorMessage(e);
      toast.error(`${message ? message : "Failed to stop all :("}`, {
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }

  /**
   * Pause all `active` and `waiting` downloads
   */
	async pauseAll() {
    try {
      const res = await secureFetch("/api/qateel/gid/pause/all", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || "Pause all action failed.");
      }

      const data = await res.json();
			/*
				On success
				json!({
					"status": "pausedAll",
					"succeeded": paused_gids,
					"failed": failed
        })
			*/
			let action = "paused all";
      if (data?.status === "pausedAll" && data?.succeeded.length > 1) {
        action += ` ${data?.succeeded.length} downloads`;
      } else if (data?.status === "pausedAll" && data?.succeeded?.length === 1) {
        action += " download";
      }

			toast.success(`Successfully ${action}!!`, {
				richColors: true,
				style: "cursor: pointer;"
			});

			if (data?.status === "pausedAll" && data?.failed.length >= 1) {
				toast.error(`Failed to pause all ${data?.failed.length} download!!`, {
					richColors: true,
					style: "cursor: pointer;"
				});
      }

			await this.getItemsCount()

    } catch (e) {
      console.error(e);
      const message = this.extractErrorMessage(e);
      toast.error(`${message ? message : "Failed to pause :("}`, {
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }

  /**
   * Resume all `paused` downloads
   */
	async resumeAll() {
    try {
      const res = await secureFetch("/api/qateel/gid/resume/all", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || "Pause action failed.");
      }

      const data = await res.json();
			/*
				On success
				json!({
					"status": "resumedAll",
					"succeeded": resumed_gids,
					"failed": failed
        })
			*/
			let action = "resumed all";
      if (data?.status === "resumedAll" && data?.succeeded.length > 1) {
        action += ` ${data?.succeeded.length} downloads`;
      } else if (data?.status === "resumedAll" && data?.succeeded?.length === 1) {
        action += " download";
      }

			toast.success(`Successfully ${action}!!`, {
				richColors: true,
				style: "cursor: pointer;"
			});

			if (data?.status === "resumedAll" && data?.failed.length >= 1) {
				toast.error(`Failed to resume all ${data?.failed.length} download!!`, {
					richColors: true,
					style: "cursor: pointer;"
				});
      }

			await this.getItemsCount();

    } catch (e) {
      console.error(e);
      const message = this.extractErrorMessage(e);
      toast.error(`${message ? message : "Failed to resume :("}`, {
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
	private itemsCountDebouncerTimer: ReturnType<typeof setTimeout> | null = null;
  async getItemsCount(): Promise<void> {
		if (this.itemsCountDebouncerTimer) {
			clearTimeout(this.itemsCountDebouncerTimer);
		}
		this.itemsCountDebouncerTimer = setTimeout(async () => {
			try {
				const res = await secureFetch("/api/qateel/user/items/count");
				const results = await res.json();
				if (!res.ok) {
					const err = results;
					throw new Error(err.error || "Failed to fetch item counts.");
				}
				itemCounts.set(results.data);
				TOTAL_ITEMS = results.data.byStatus.all;
			} catch (e: any) {
				console.error(e);
				const message = this.extractErrorMessage(e)
				toast.error(`${message ? message : "Failed to fetch item counts."}`, {
					closeButton: true,
					richColors: true,
				});
			}
		}, 250)
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

	async purgeAll() {
    try {
      const res = await secureFetch("/api/qateel/purge/all", {
        method: "POST",
        headers: { "Content-Type": "application/json" },
      });

      if (!res.ok) {
        const err = await res.json();
        throw new Error(err.error || "Purge all action failed.");
      }

      const data = await res.json();

      if (data?.status === "purgedAll") {
				toast.success(`Successfully purged all complete, error, removed downloads!!`, {
					richColors: true,
					style: "cursor: pointer;"
				});
      }

			await this.getItemsCount();

    } catch (e) {
      console.error(e);
      const message = this.extractErrorMessage(e);
      toast.error(`${message ? message : "Failed to purge :("}`, {
        richColors: true,
        style: "cursor: pointer;"
      });
    }
  }


  /// Convert incoming `TickTask` to standard `ItemMetaData`
	private tickTaskToItemMetaData(task: TickTask): ItemMetaData {
		return {
			gid: task.gid,
			name: task.name,
			kind: task.kind,
			category: null,
			status: task.status,
			dir: task.dir,
			files: task.files ?? "[]",
			totalLength: task.totalLength,
			completedLength: task.completedLength,
			uploadLength: task.uploadLength,
			sourceUri: task.sourceUri,
			infoHash: null,
			seeder: task.seeder ?? null,
			contentHash: null,
			errorCode: null,
			errorMessage: null,
			torrent: null,
			options: null,
			isResumeSupported: task.isResumeSupported ?? null,
			numPieces: null,
			pieceLength: null,
			verifiedLength: task.verifiedLength ?? null,
			verifyIntegrityPending: task.verifyIntegrityPending ?? null,
			createdAt: new SvelteDate().toISOString(),
			completedAt: null,
			downloadSpeed: task.downloadSpeed,
			uploadSpeed: task.uploadSpeed,
			numSeeders: task.numSeeders ?? null,
			connections: task.connections ?? null,
		};
	}

	/// Check if item matches the active status and category filters
	private matchesCurrentFilter(item: {
		status: GidStatus;
		category?: any;
		name?: string | null;
		files?: string | null;
		kind?: Kind;
	}): boolean {
		try {
			const currentStatus = get(selectedStatus);
			if (currentStatus && currentStatus !== "all" && item.status !== currentStatus) {
				return false;
			}

			const currentCat = get(selectedCategory);
			if (currentCat && currentCat !== "everything") {
				const itemCat = getItemCategory(item as ItemMetaData);
				if (itemCat !== currentCat) {
					return false;
				}
			}

			return true;
		} catch {
			return true;
		}
	}

	/// Compare two items for sorting
	private compareItems(a: ItemMetaData, b: ItemMetaData, sortBy: SortBy = "time", order: Order = "desc"): number {
		let diff = 0;
		if (sortBy === "name") {
			const nameA = a.name || a.gid || "";
			const nameB = b.name || b.gid || "";
			diff = nameA.localeCompare(nameB, undefined, { sensitivity: "base", numeric: true });
		} else if (sortBy === "size") {
			const sizeA = parseInt(a.totalLength || "0", 10) || 0;
			const sizeB = parseInt(b.totalLength || "0", 10) || 0;
			diff = sizeA - sizeB;
		} else {
			// Time
			const timeA = a.createdAt ? new SvelteDate(a.createdAt).getTime() : 0;
			const timeB = b.createdAt ? new SvelteDate(b.createdAt).getTime() : 0;
			diff = timeA - timeB;
		}

		return order === "desc" ? -diff : diff;
	}

	/// Binary search insert to maintain sort order
	private insertSortedItem(items: ItemMetaData[], newItem: ItemMetaData, sortBy: SortBy = "time", order: Order = "desc") {
		if (items.some(i => i.gid === newItem.gid)) return;

		let low = 0;
		let high = items.length;

		while (low < high) {
			const mid = (low + high) >>> 1;
			if (this.compareItems(newItem, items[mid], sortBy, order) < 0) {
				high = mid;
			} else {
				low = mid + 1;
			}
		}

		items.splice(low, 0, newItem);
	}

	/**
		* Calculates the ETA and returns it as a formatted string
		*/
	calculateETA(
		item: ItemMetaData
	): string {
		const speedHistoryCache = new WeakMap<ItemMetaData, SpeedRecord[]>();
		const totalBytes = item.totalLength ? parseInt(item.totalLength, 10) : 0;
		const completedBytes = item.completedLength ? parseInt(item.completedLength, 10) : 0;
		const speedBytesPerSec = item.downloadSpeed ? parseInt(item.downloadSpeed, 10) : 0;

		const remaining = totalBytes - completedBytes;
		// Immediate termination logic for edge cases
		if (remaining <= 0) return "0s";

		// Initialize or retrieve the history array
		if (!speedHistoryCache.has(item)) {
			speedHistoryCache.set(item, []);
		}
		const history = speedHistoryCache.get(item)!;

		const now = Date.now();
		history.push({ timestamp: now, speed: speedBytesPerSec });

		// Maintain the 10 sec history window
		const cutoff = now - 10000;
		while (history.length > 0 && history[0].timestamp < cutoff) {
			history.shift();
		}

		const totalSpeed = history.reduce((sum, entry) => sum + entry.speed, 0);
		const averageSpeed = totalSpeed / history.length;

		if (averageSpeed <= 0) return "";

		const totalSeconds = Math.round(remaining / averageSpeed);

		const MINUTE = 60;
		const HOUR = 3600;
		const DAY = 86400;

		if (totalSeconds >= DAY) {
			const days = Math.floor(totalSeconds / DAY);
			const hours = Math.floor((totalSeconds % DAY) / HOUR);
			return hours > 0 ? `${days}d ${hours}h` : `${days}day`;
		}

		if (totalSeconds >= HOUR) {
			const hours = Math.floor(totalSeconds / HOUR);
			const mins = Math.floor((totalSeconds % HOUR) / MINUTE);
			return mins > 0 ? `${hours}h ${mins}m` : `${hours}hour`;
		}

		if (totalSeconds >= MINUTE) {
			const mins = Math.floor(totalSeconds / MINUTE);
			const secs = totalSeconds % MINUTE;
			return secs > 0 ? `${mins}m ${secs}s` : `${mins}min`;
		}

		return `${totalSeconds}sec`;
	}
}

export const aria2 = new Aria2Manager();


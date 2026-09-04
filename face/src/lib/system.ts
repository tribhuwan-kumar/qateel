import { toast } from "svelte-sonner";
import { writable } from "svelte/store";
import { browser } from "$app/environment";

export interface SystemStatus {
  version: string;
  adminExists: boolean;
  aria2Alive: boolean;
  wriaAlive: boolean;
}

export const systemState = writable<{
  status: SystemStatus | null;
}>({
  status: null
});

let ws: WebSocket | null = null;

export function getSysStatus(): void {
  if (!browser || ws) return;

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
  const url = `${protocol}//${window.location.host}/api/ws/status`;

  ws = new WebSocket(url);

  ws.onopen = () => {
    systemState.update(state => ({
      ...state,
      status: state.status ? { ...state.status, wriaAlive: true } : null
    }));
  };

  ws.onmessage = (event) => {
    try {
      const data: SystemStatus = JSON.parse(event.data);
      console.log("Connected to sys status stream:", data);
      systemState.set({ status: { ...data, wriaAlive: true }});

      if (data.aria2Alive === false) {
        toast.error("Aria2 daemon isn't running ` ᴖ ´",{
          duration: 10000,
          richColors: true,
          style: "cursor: pointer;",
        })
      }
    } catch (e) {
      console.error("getSysStatus:", e)
    }
  };

  let retryMs = 1000;
  let reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  ws.onerror = () => {
    systemState.update(s => ({
      ...s,
      status: s.status ? { ...s.status, wriaAlive: false } : null
    }));
    toast.error("Qateel isn't alive ˙◠˙",{
      duration: 3000,
      richColors: true,
      style: "cursor: pointer;",
    })
  };

  ws.onclose = () => {
    systemState.update(state => ({
      ...state,
      status: state.status ? { ...state.status, wriaAlive: false } : null
    }));
    toast.error("Qateel isn't alive ˙◠˙",{
      duration: 10000,
      richColors: true,
      style: "cursor: pointer;",
    })
    ws = null;
    setTimeout(getSysStatus, 11000);
  };

  ws.onclose = () => {
    ws = null;
    if (!reconnectTimer) {
      reconnectTimer = setTimeout(() => {
        reconnectTimer = null;
        getSysStatus();
      }, retryMs);
      retryMs = Math.min(retryMs * 2, 30000); // backoff
    }
  };
}


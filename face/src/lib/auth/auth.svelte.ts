import { goto } from "$app/navigation";
import { secureFetch } from "$lib/utils";
import { browser } from "$app/environment";

let authIntervalId: ReturnType<typeof setInterval> | null = null;

interface Me {
  "role": string | null,
  "userId": string | null,
  "username": string | null,
	"authenticated": boolean | null,
}

export const authState: Me = $state({
  role: null as string | null,
  username: null as string | null,
  userId: null as string | null,
  authenticated: false,
});

export async function checkAuth() {
  if (!browser) return;
  try {
    const res = await secureFetch("/api/auth/me");
    if (res.ok) {
      const data: Me = await res.json();
      console.log("Current user info:", data);
			authState.role = data.role;
			authState.userId = data.userId;
			authState.username = data.username;
			authState.authenticated = data.authenticated;
      return true;
    } else {
      clearAuth();
      return false;
    }
  } catch (e) {
    console.error("Auth check failed", e);
    return false;
  }
}

export async function logout() {
  await secureFetch("/api/auth/logout", { method: "POST" });
  clearAuth();
}

function clearAuth() {
  authState.role = null;
  authState.userId = null;
  authState.username = null;
  authState.authenticated = false;

  if (browser && window.location.pathname !== '/') {
    goto('/');
		window.location.href = "/";
  }
}

export function startAuthWorker() {
  if (!browser || authIntervalId) return;
  checkAuth();

  authIntervalId = setInterval(async () => {
    const isAuthed = await checkAuth();

    if (!isAuthed && window.location.pathname !== '/') {
      clearAuth();
    }
  }, 5000);
}

export function stopAuthWorker() {
  if (authIntervalId) {
    clearInterval(authIntervalId);
    authIntervalId = null;
  }
}

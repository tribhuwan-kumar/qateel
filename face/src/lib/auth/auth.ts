import { writable } from "svelte/store";
import { goto } from '$app/navigation';
import { browser } from "$app/environment";
import { secureFetch } from "$lib/utils";

export const authState = writable<{
  role: string | null;
  username: string | null;
  userId: string | null,
  authenticated: boolean;
}>({
  role: null,
  username: null,
	userId: null,
  authenticated: false,
});

interface Me {
  "role": string,
  "userId": string,
  "username": string
	"authenticated": boolean,
}

export async function checkAuth() {
  if (!browser) return;
  try {
    const res = await secureFetch("/api/auth/me");
    if (res.ok) {
      const data: Me = await res.json();
      console.log("Current user info:", data);
      authState.set({
				role: data.role,
				userId: data.userId,
        username: data.username,
				authenticated: data.authenticated,
      });
      return true;
    } else {
      // Cookie invalid or missing
      authState.set({
					role: null,
					userId: null,
					username: null,
					authenticated: false,
			});
      return false;
    }
  } catch (e) {
    console.error("Auth check failed", e);
    return false;
  }
}

export async function logout() {
  await secureFetch("/api/auth/logout", {
		method: "POST"
	});
	authState.set({
			role: null,
			userId: null,
			username: null,
			authenticated: false,
	});
	goto('/');
  window.location.href = "/";
}

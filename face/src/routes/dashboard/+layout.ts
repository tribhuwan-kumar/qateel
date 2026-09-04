import { browser } from '$app/environment';
import { redirect } from '@sveltejs/kit';
import { checkAuth } from '$lib/auth/auth.svelte';

// Disable SSR for the dashboard. svelteKit will send an empty shell!!
// and load function before painting the DOM, completely eliminating FOUC.
export const ssr = false;

export async function load() {
	if (browser) {
		const isLoggedIn = await checkAuth();
		console.log("isLoggedIn", isLoggedIn);
		if (!isLoggedIn) {
			redirect(302, '/');
		}
	}
	return {};
}


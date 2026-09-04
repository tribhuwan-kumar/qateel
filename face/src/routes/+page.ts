import { redirect } from "@sveltejs/kit";
import { browser } from '$app/environment';
import { checkAuth } from "$lib/auth/auth.svelte";

export const ssr = false;

export async function load() {
	if (browser) {
		const isLoggedIn = await checkAuth();
		if (isLoggedIn) {
			redirect(302, '/dashboard');
		}
	}
	return {};
}

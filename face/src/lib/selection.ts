import { writable, derived, get } from 'svelte/store';

// Map of GID -> totalLength (in bytes)
export const selectedMap = writable<Map<string, number>>(new Map());
// Active drag-selection state (buffers WebSocket row insertions while dragging)
export const isActivelySelecting = writable<boolean>(false);

export function parseBytes(val: string | number | null | undefined): number {
	if (typeof val === 'number') return isNaN(val) ? 0 : Math.max(0, val);
	if (!val) return 0;
	const n = parseInt(val, 10);
	return isNaN(n) ? 0 : Math.max(0, n);
}

export const selectedTotalBytes = derived(selectedMap, ($map) => {
	let sum = 0;
	for (const bytes of $map.values()) {
		sum += bytes;
	}
	return sum;
});

// Backwards-compatible store returning string[] of selected GIDs
export const selectedGids = {
	subscribe: derived(selectedMap, ($map) => Array.from($map.keys())).subscribe,
		set: (gids: string[]) => {
		selectedMap.update((current) => {
			const next = new Map<string, number>();
			for (const gid of gids) {
				next.set(gid, current.get(gid) ?? 0);
			}
			return next;
		});
	},
	update: (fn: (current: string[]) => string[]) => {
		selectedMap.update((current) => {
			const currentArr = Array.from(current.keys());
			const nextArr = fn(currentArr);
			const next = new Map<string, number>();
			for (const gid of nextArr) {
				next.set(gid, current.get(gid) ?? 0);
			}
			return next;
		});
	}
};

export function isSelected(gid: string): boolean {
	return get(selectedMap).has(gid);
}

export function toggleSelection(gid: string, totalLength?: string | number | null) {
	selectedMap.update((current) => {
		const next = new Map(current);
		if (next.has(gid)) {
			next.delete(gid);
		} else {
			next.set(gid, parseBytes(totalLength));
		}
		return next;
	});
}

export function selectExclusive(gid: string, totalLength?: string | number | null) {
	selectedMap.update((current) => {
		if (current.size === 1 && current.has(gid)) {
			return new Map();
		}
		const next = new Map<string, number>();
		next.set(gid, parseBytes(totalLength));
		return next;
	});
}

export function setSelection(items: Array<{ gid: string; totalLength?: string | number | null }>) {
	const next = new Map<string, number>();
	for (const item of items) {
		if (item.gid) {
			next.set(item.gid, parseBytes(item.totalLength));
		}
	}
	selectedMap.set(next);
}

export function addSelection(items: Array<{ gid: string; totalLength?: string | number | null }>) {
	selectedMap.update((current) => {
		const next = new Map(current);
		for (const item of items) {
			if (item.gid) {
				next.set(item.gid, parseBytes(item.totalLength));
			}
		}
		return next;
	});
}

export function clearSelection() {
	selectedMap.set(new Map());
}


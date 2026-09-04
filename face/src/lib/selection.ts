import { writable } from 'svelte/store';

export const selectedGids = writable<string[]>([]);

// GID selection
export function toggleSelection(gid: string) {
  selectedGids.update(current => {
    if (current.includes(gid)) return current.filter(id => id !== gid);
    return [...current, gid];
  });
}

export function selectExclusive(gid: string) {
  selectedGids.update(current => {
    if (current.length === 1 && current[0] === gid) return [];
    return [gid];
  });
}

export function clearSelection() {
  selectedGids.set([]);
}


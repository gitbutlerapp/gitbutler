import { get, writable } from '@square/svelte-store';
import { browser } from '$app/environment';
import { CloudApi } from '$lib/api';

const cloud = CloudApi();

export const store = writable<Map<string, string>>(
	(browser && new Map(Object.entries(JSON.parse(localStorage.getItem('hunkSummaries') || '{}')))) ||
		new Map<string, string>()
);

store.subscribe((val: Map<string, string>) => {
	if (browser) {
		localStorage.setItem('hunkSummaries', JSON.stringify(Object.fromEntries(val)));
	}
});

export async function summarizeHunk(diff: string): Promise<string> {
	const cache = get(store);
	const diffHash = hash(diff);

	if (cache.has(diffHash)) {
		return cache.get(diffHash) as string;
	}

	const rsp = await cloud.summarize.hunk({ hunk: diff });
	cache.set(diffHash, rsp.message);
	store.set(cache);
	return rsp.message;
}

function hash(s: string) {
	let h = 0;
	let i = s.length;
	while (i > 0) {
		h = ((h << 5) - h + s.charCodeAt(--i)) | 0;
	}
	return h.toString();
}

import { subscribe } from '$lib/api/git/fetches';
import { writable } from '@square/svelte-store';

export function getFetchesStore(projectId: string) {
	const store = writable<any>([]);
	// TODO: We need to unsubscribe this!
	const unsubscribe = subscribe({ projectId }, (result) => {
		store.set(result);
	});
	return { store, unsubscribe };
}

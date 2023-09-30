import { subscribe } from '$lib/api/git/fetches';
import { writable, type Loadable } from '@square/svelte-store';

export function getFetchesStore(projectId: string): Loadable<any> {
	let counter = 0; // Store doesn't emit unless it gets new value
	return writable<any>([], (set) => {
		const unsubscribe = subscribe(projectId, () => set(counter++));
		return () => unsubscribe();
	});
}

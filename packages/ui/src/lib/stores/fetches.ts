import { subscribe } from '$lib/api/git/fetches';
import { writable, type Readable } from '@square/svelte-store';

export interface FetchesStore extends Readable<any[]> {
	subscribeStream(): () => void;
}

export function getFetchesStore(projectId: string): FetchesStore {
	const store = writable<any>([]);
	let counter = 0; // Store doesn't emit unless it gets new value
	const subscribeStream = () => {
		return subscribe({ projectId }, () => {
			store.set(counter++);
		});
	};
	return { ...store, subscribeStream };
}

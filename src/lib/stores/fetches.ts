import { subscribe } from '$lib/api/git/fetches';
import { writable, type Readable } from '@square/svelte-store';

export interface FetchesStore extends Readable<any[]> {
	subscribeStream(): () => void;
}

export function getFetchesStore(projectId: string): FetchesStore {
	const store = writable<any>([]);
	// TODO: We need to unsubscribe this!
	const subscribeStream = () => {
		return subscribe({ projectId }, (result) => {
			store.set(result);
		});
	};
	return { ...store, subscribeStream };
}

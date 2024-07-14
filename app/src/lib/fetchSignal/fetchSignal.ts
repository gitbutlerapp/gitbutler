import { listen } from '$lib/backend/ipc';
import { readable } from 'svelte/store';

export class FetchSignal {
	readonly event = readable<number>(undefined, (set) => {
		const unsubscribe = subscribeToFetches(this.projectId, () => set(this.counter++));
		return () => {
			unsubscribe();
		};
	});

	private counter = 0;

	constructor(private projectId: string) {}
}

export function subscribeToFetches(projectId: string, callback: () => Promise<void> | void) {
	return listen<any>(`project://${projectId}/git/fetch`, callback);
}

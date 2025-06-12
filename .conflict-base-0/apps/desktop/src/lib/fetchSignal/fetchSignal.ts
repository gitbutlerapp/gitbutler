import { listen } from '$lib/backend/ipc';
import { readable } from 'svelte/store';

export class FetchSignal {
	// Stores only emit unique values so we use a counter to ensure
	// derived stores are updated.
	private counter = 0;

	// Emits a new value when a fetch was detected by the back end.
	readonly event = readable<number>(undefined, (set) => {
		const unsubscribe = listen<any>(`project://${this.projectId}/git/fetch`, () =>
			set(this.counter++)
		);
		return async () => await unsubscribe();
	});

	constructor(private projectId: string) {}
}

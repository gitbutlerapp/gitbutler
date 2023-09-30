import { writable } from '@square/svelte-store';
import * as deltas from '$lib/api/ipc/deltas';
import { get, type Writable } from '@square/svelte-store';
import type { Delta } from '$lib/api/ipc/deltas';

export function getDeltasStore(
	projectId: string,
	sessionId: string | undefined = undefined
): Writable<Partial<Record<string, Delta[]>>> & { setSessionId: (sessionId: string) => void } {
	// We have a special situation here where we use deltas to know when to re-run
	// list_virtual_branches. We therefore have to be able to update the store rather
	// than creating a new one, the virtualBranchStore "depends" on the delta store.

	// Deefault no-op initial unsubscriber
	let unsubscribe: () => void = () => undefined;

	const store = writable<Partial<Record<string, Delta[]>>>(undefined, (set) => {
		sessionId && deltas.list({ projectId, sessionId }).then(set);
		return () => unsubscribe();
	});

	return {
		...store,
		setSessionId: (sid) => {
			sessionId = sid;
			unsubscribe();
			deltas.list({ projectId, sessionId }).then(store.set);
			unsubscribe = deltas.subscribe(
				{ projectId, sessionId },
				({ filePath, deltas: newDeltas }) => {
					const oldValue = get(store);
					store.set({
						...oldValue,
						[filePath]: [...(oldValue[filePath] || []), ...newDeltas]
					});
				}
			);
		}
	};
}

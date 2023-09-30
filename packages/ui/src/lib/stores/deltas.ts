import { asyncWritable, isReloadable, writable } from '@square/svelte-store';
import * as deltas from '$lib/api/ipc/deltas';
import type { Delta } from '$lib/api/ipc/deltas';
import type { Stores, Writable } from 'svelte/store';

export function getDeltasStore(
	projectId: string,
	sessionId: string | undefined = undefined
): Writable<Partial<Record<string, Delta[]>>> & { setSessionId: (sid: string) => void } {
	// We have a special situation here where we use deltas to know when to re-run
	// list_virtual_branches. We therefore have to be able to update the store rather
	// than creating a new one, the virtualBranchStore "depends" on the delta store.

	let unsubscribe: () => void;

	const store = asyncWritable<Stores, Partial<Record<string, Delta[]>>>(
		[],
		async () => {
			if (!sessionId) return {};
			if (unsubscribe) unsubscribe();
			unsubscribe = deltas.subscribe(projectId, sessionId, ({ filePath, deltas: newDeltas }) => {
				store.update((storeValue) => {
					storeValue[filePath] = [...(storeValue[filePath] || []), ...newDeltas];
					return storeValue;
				});
			});
			return await deltas.list({ projectId, sessionId });
		},
		undefined,
		{ reloadable: true },
		() => {
			return () => unsubscribe();
		}
	);

	return {
		...store,
		setSessionId: (sid) => {
			sessionId = sid;
			isReloadable(store) && store.reload();
		}
	};
}

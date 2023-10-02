import { asyncWritable, isReloadable } from '@square/svelte-store';
import { subscribeToDeltas, type Delta, listDeltas } from '$lib/api/ipc/deltas';
import type { Stores, Writable } from 'svelte/store';

/**
 * We have a special situation here where we use deltas to know when to re-run
 * list_virtual_branches, but session ids change so we need the ability to update
 * the store rather than creating a new one. The delta store is passed as a dependent
 * to the virtualBranchStore to which triggers a re-run of the load function.
 */
export function getDeltasStore(
	projectId: string,
	sessionId: string | undefined = undefined
): Writable<Partial<Record<string, Delta[]>>> & { setSessionId: (sid: string) => void } {
	let unsubscribe: () => void;
	const store = asyncWritable<Stores, Partial<Record<string, Delta[]>>>(
		[],
		async () => {
			if (!sessionId) return {};
			if (unsubscribe) unsubscribe();
			unsubscribe = subscribeToDeltas(projectId, sessionId, ({ filePath, deltas }) => {
				store.update((storeValue) => {
					storeValue[filePath] = [...(storeValue[filePath] || []), ...deltas];
					return storeValue;
				});
			});
			return await listDeltas({ projectId, sessionId });
		},
		undefined,
		{ reloadable: true },
		() => {
			return () => unsubscribe && unsubscribe();
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

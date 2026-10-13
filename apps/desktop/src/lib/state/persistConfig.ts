import autoMergeLevel1 from "redux-persist/lib/stateReconciler/autoMergeLevel1";
import storage from "redux-persist/lib/storage";
import type { PersistConfig } from "redux-persist";

/**
 * Persist configuration for a slice, keeping the `blacklist`ed keys out of storage.
 *
 * `blacklist` on its own only stops those keys being written. Whatever an earlier build already
 * wrote is still read back and merged on the next launch, so the reconciler drops them on the
 * way in as well; without that, the first launch after upgrading still restores stale state.
 */
export function persistConfigFor<S extends object>(
	key: string,
	blacklist?: Extract<keyof S, string>[],
): PersistConfig<S> {
	if (!blacklist || blacklist.length === 0) {
		return { key, storage };
	}
	return {
		key,
		storage,
		blacklist,
		stateReconciler: (inbound: S, original: S, reduced: S, config: PersistConfig<S>) => {
			const kept = { ...inbound };
			for (const name of blacklist) {
				delete kept[name];
			}
			return autoMergeLevel1(kept, original, reduced, config);
		},
	};
}

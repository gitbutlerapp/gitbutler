import { invoke } from '$lib/ipc';
import { git } from '$lib/api/ipc';
import type { Target } from './types';
import { error } from '$lib/toasts';
import { writable, type Loadable } from 'svelte-loadable-store';
import type { Writable, Readable } from '@square/svelte-store';

const cache: Map<string, TargetOperations & Readable<Loadable<Target>>> = new Map();

export interface TargetOperations {
	refresh(): Promise<void | object>;
}

export function getTarget(projectId: string): TargetOperations & Readable<Loadable<Target>> {
	const cachedStore = cache.get(projectId);
	if (cachedStore) {
		return cachedStore;
	}
	const writeable = createWriteable(projectId);

	const store: TargetOperations & Readable<Loadable<Target>> = {
		refresh: () => refresh(projectId, writeable),
		subscribe: writeable.subscribe
	};

	cache.set(projectId, store);
	return store;
}

async function getTargetData(projectId: string) {
	return invoke<Target>('get_target_data', { projectId });
}

function createWriteable(projectId: string) {
	return writable(getTargetData(projectId), (set) => {
		git.fetches.subscribe({ projectId }, () => {
			getTargetData(projectId).then((newTarget) => {
				set(newTarget);
			});
		});
	});
}

function refresh(projectId: string, store: Writable<Loadable<Target>>) {
	return getTargetData(projectId).then((newTarget) =>
		store.set({ isLoading: false, value: newTarget })
	);
}

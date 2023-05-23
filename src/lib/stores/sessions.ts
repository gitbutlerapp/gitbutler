import { writable, type Loadable } from 'svelte-loadable-store';
import { sessions, type Session } from '$lib/api';
import { get, type Readable } from '@square/svelte-store';

const stores: Record<string, Readable<Loadable<Session[]>>> = {};

export default (params: { projectId: string }) => {
	if (params.projectId in stores) return stores[params.projectId];

	const store = writable(sessions.list(params), (set) =>
		sessions.subscribe(params, ({ session }) => {
			const oldValue = get(store);
			if (oldValue.isLoading) {
				sessions.list(params).then(set);
			} else {
				set(
					oldValue.value
						.filter((b) => b.id !== session.id)
						.concat({
							projectId: params.projectId,
							...session
						})
				);
			}
		})
	);
	stores[params.projectId] = store;
	return store as Readable<Loadable<Session[]>>;
};

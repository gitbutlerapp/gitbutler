import { asyncWritable, type Loadable } from '@square/svelte-store';
import { checkUpdate, installUpdate } from '@tauri-apps/api/updater';

export type Update = { enabled: boolean; shouldUpdate?: boolean; body?: string; version?: string };

export function newUpdateStore(): Loadable<Update> {
	const updateStore = asyncWritable(
		[],
		async () => {
			const update = await checkUpdate();
			if (update === undefined) {
				return { enabled: false };
			} else if (!update.shouldUpdate) {
				return { enabled: true, shouldUpdate: false };
			} else {
				return {
					enabled: true,
					shouldUpdate: true,
					body: update.manifest!.body,
					version: update.manifest!.version
				};
			}
		},
		async (data) => data,
		{ trackState: true, reloadable: true }
	);

	setInterval(() => {
		// Check for updates every 12h
		if (updateStore.reload) updateStore.reload();
	}, 43200 * 1000);

	return updateStore;
}

export async function install() {
	await installUpdate();
}

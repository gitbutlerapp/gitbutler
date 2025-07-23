import type { Tauri } from '$lib/backend/tauri';

export class GitService {
	constructor(private tauri: Tauri) {}

	/**
	 * Emits a new value when a fetch was detected by the back end.
	 * @example
	 * $effect(() => gitService.onFetch(data.projectId, () => {}));
	 */
	onFetch(projectId: string, callback: () => void) {
		return this.tauri.listen<any>(`project://${projectId}/git/fetch`, callback);
	}
}

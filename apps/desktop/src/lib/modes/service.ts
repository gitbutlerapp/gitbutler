import { invoke, listen } from '$lib/backend/ipc';
import { derived, writable } from 'svelte/store';

type Mode = { type: 'OpenWorkspace' } | { type: 'OutsideWorksapce' } | { type: 'Edit' };

export class ModeService {
	readonly head = writable<string | undefined>(undefined);
	readonly mode = writable<Mode | undefined>(undefined);

	readonly gbBranchActive = derived(this.head, (head) => head === 'gitbutler/integration');

	unsubscribe?: () => Promise<void>;

	constructor(private projectId: string) {
		this.unsubscribe = subscribeToHead(projectId, (head, mode) => {
			this.head.set(head);
			this.mode.set(mode);
		});
		this.refresh();
	}

	private async refresh() {
		const head = await invoke<string>('git_head', { projectId: this.projectId });
		this.head.set(head);

		const mode = await invoke<Mode>('operating_mode', { projectId: this.projectId });
		this.mode.set(mode);
	}
}

function subscribeToHead(projectId: string, callback: (head: string, mode: Mode) => void) {
	return listen<{ head: string; operating_mode: Mode }>(
		`project://${projectId}/git/head`,
		(event) => callback(event.payload.head, event.payload.operating_mode)
	);
}

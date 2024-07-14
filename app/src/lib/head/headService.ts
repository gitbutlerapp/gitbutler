import { invoke, listen } from '$lib/backend/ipc';
import { derived, writable } from 'svelte/store';

export class HeadService {
	readonly head = writable<string>(undefined, () => {
		this.refresh();
		this.unsubscribe = subscribeToHead(this.projectId, (head) => this.head.set(head));
		return () => {
			this.unsubscribe?.();
		};
	});

	readonly gbBranchActive = derived(this.head, (head) => head === 'gitbutler/integration');

	unsubscribe?: () => Promise<void>;

	constructor(private projectId: string) {}

	private async refresh() {
		let head = await invoke<string>('git_head', { projectId: this.projectId });
		head = head.replace('refs/heads/', '');
		this.head.set(head);
	}
}

function subscribeToHead(projectId: string, callback: (head: string) => void) {
	return listen<{ head: string }>(`project://${projectId}/git/head`, (event) =>
		callback(event.payload.head.replace('refs/heads/', ''))
	);
}

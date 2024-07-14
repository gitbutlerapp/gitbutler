import { invoke, listen } from '$lib/backend/ipc';
import { derived, readable } from 'svelte/store';

export class HeadService {
	constructor(private projectId: string) {}

	readonly name = readable<string>(undefined, (set) => {
		this.getHead(this.projectId).then((head) => set(head));
		const unsubscribe = subscribeToHead(this.projectId, (head) => set(head));
		return () => {
			unsubscribe();
		};
	});

	readonly gbBranchActive = derived(this.name, (head) => head === 'gitbutler/integration');

	private async getHead(projectId: string) {
		const head = await invoke<string>('git_head', { projectId });
		return head.replace('refs/heads/', '');
	}
}

function subscribeToHead(projectId: string, callback: (head: string) => void) {
	return listen<{ head: string }>(`project://${projectId}/git/head`, (event) =>
		callback(event.payload.head.replace('refs/heads/', ''))
	);
}

import { invoke } from '$lib/backend/ipc';
import { showError } from '$lib/notifications/toasts';

export class RemotesService {
	async remotes(projectId: string) {
		return await invoke<string[]>('list_remotes', { projectId });
	}

	async addRemote(projectId: string, name: string, url: string) {
		try {
			await invoke('add_remote', { projectId, name, url });
		} catch (e) {
			showError('Failed to add remote', e);
		}
	}
}

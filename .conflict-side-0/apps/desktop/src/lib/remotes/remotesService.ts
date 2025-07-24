import { invoke } from '$lib/backend/ipc';
import { InjectionToken } from '@gitbutler/shared/context';

export interface GitRemote {
	name?: string;
	url?: string;
}

export const REMOTES_SERVICE_TOKEN = new InjectionToken<RemotesService>('RemotesService');

export class RemotesService {
	async remotes(projectId: string) {
		return await invoke<GitRemote[]>('list_remotes', { projectId });
	}

	async addRemote(projectId: string, name: string, url: string) {
		const remotes = await this.remotes(projectId);

		const sameNameRemote = remotes.find((remote) => remote.name === name);
		if (sameNameRemote) {
			throw new Error(`Remote with name ${sameNameRemote.name} already exists.`);
		}

		const sameUrlRemote = remotes.find((remote) => remote.url === url);
		if (sameUrlRemote) {
			// This should not happen, and indicates we are incorrectly showing an "apply from fork"
			// button in the user interface.
			throw new Error(`Remote ${sameUrlRemote.name} with url ${sameUrlRemote.url} already exists.`);
		}

		return await invoke<string>('add_remote', { projectId, name, url });
	}
}

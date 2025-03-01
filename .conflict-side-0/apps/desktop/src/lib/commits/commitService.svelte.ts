import { Commit } from './commit';
import { invoke } from '$lib/backend/ipc';
import { plainToInstance } from 'class-transformer';

export class CommitService {
	async find(projectId: string, commitOid: string) {
		return plainToInstance(Commit, await invoke<Commit>('find_commit', { projectId, commitOid }));
	}
}

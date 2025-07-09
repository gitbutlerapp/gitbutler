import { invoke } from '$lib/backend/ipc';
import { Commit } from '$lib/commits/commit';
import { plainToInstance } from 'class-transformer';

export class CommitService {
	async find(projectId: string, commitId: string) {
		return plainToInstance(Commit, await invoke<Commit>('find_commit', { projectId, commitId }));
	}
}

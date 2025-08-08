import { invoke } from '$lib/backend/ipc';
import { Commit } from '$lib/commits/commit';
import { InjectionToken } from '@gitbutler/shared/context';
import { plainToInstance } from 'class-transformer';

export const COMMIT_SERVICE = new InjectionToken<CommitService>('CommitService');

export class CommitService {
	async find(projectId: string, commitId: string) {
		return plainToInstance(Commit, await invoke<Commit>('find_commit', { projectId, commitId }));
	}
}

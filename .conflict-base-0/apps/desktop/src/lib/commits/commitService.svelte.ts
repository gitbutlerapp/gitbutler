import { Commit } from '$lib/commits/commit';
import { InjectionToken } from '@gitbutler/shared/context';
import { plainToInstance } from 'class-transformer';
import type { IBackend } from '$lib/backend';

export const COMMIT_SERVICE = new InjectionToken<CommitService>('CommitService');

export class CommitService {
	constructor(private backend: IBackend) {}
	async find(projectId: string, commitId: string) {
		return plainToInstance(
			Commit,
			await this.backend.invoke<Commit>('find_commit', { projectId, commitId })
		);
	}
}

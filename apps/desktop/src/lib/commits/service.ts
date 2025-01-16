import { Commit } from './commit';
import { invoke } from '$lib/backend/ipc';
import { plainToInstance } from 'class-transformer';

export class CommitService {
	constructor(private projectId: string) {}

	async find(commitOid: string): Promise<Commit> {
		const maybeCommit = await this.findMaybe(commitOid);
		if (maybeCommit === undefined) {
			throw new Error('Commit not found');
		}
		return maybeCommit;
	}

	async findMaybe(commitOid: string): Promise<Commit | undefined> {
		const commit = await invoke<unknown>('find_commit', { projectId: this.projectId, commitOid });
		return plainToInstance(Commit, commit);
	}
}

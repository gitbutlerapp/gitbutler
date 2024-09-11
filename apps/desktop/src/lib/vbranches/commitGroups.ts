import { DetailedCommit } from './types';

class CommitGroup {
	commits: DetailedCommit[];

	constructor(readonly ref?: string | undefined) {
		this.commits = [];
	}

	get localCommits() {
		return this.commits.filter((c) => c.status === 'local');
	}

	get remoteCommits() {
		return this.commits.filter((c) => c.status === 'localAndRemote');
	}

	get integratedCommits() {
		return this.commits.filter((c) => c.status === 'integrated');
	}

	get branchName() {
		return this.ref?.replace('refs/remotes/origin/', '');
	}
}

export function groupCommitsByRef(arr: DetailedCommit[]): CommitGroup[] {
	const groups: CommitGroup[] = [];
	let currentGroup: CommitGroup | undefined;

	for (const item of arr) {
		if (item.remoteRef || currentGroup === undefined) {
			currentGroup = new CommitGroup(item.remoteRef);
			groups.push(currentGroup);
		}
		currentGroup.commits.push(item);
	}
	return groups;
}

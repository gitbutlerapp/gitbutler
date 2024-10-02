/**
 * This will be removed as soon as Kiril has landed the stacking api.
 */
import { DetailedCommit } from './types';

// If no remoteRef exists on the first commit, create a fake one.
export const AUTO_GROUP = 'auto-group';

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

	for (let i = 0; i < arr.length; i++) {
		const item = arr[i] as DetailedCommit; // Becasuse of noUncheckedIndexedAccess.
		const remoteRef = i === 0 ? item.remoteRef || AUTO_GROUP : item.remoteRef;
		if (remoteRef || currentGroup === undefined) {
			currentGroup = new CommitGroup(remoteRef);
			groups.push(currentGroup);
		}
		currentGroup.commits.push(item);
	}
	return groups;
}

import { HunkLock, type Commit } from './types';
import { unique } from '$lib/utils/filters';

export function getLockText(hunkLocks: HunkLock | HunkLock[] | string, commits: Commit[]): string {
	if (!hunkLocks || commits === undefined) return 'Depends on a committed change';

	const locks = hunkLocks instanceof HunkLock ? [hunkLocks] : (hunkLocks as HunkLock[]);

	const descriptions = locks
		.filter(unique)
		.map((lock) => {
			const commit = commits.find((c) => {
				return c.id === lock.commitId;
			});
			const shortCommitId = commit?.id.slice(0, 7);
			if (commit) {
				const shortTitle = commit.descriptionTitle?.slice(0, 35) + '...';
				return `"${shortTitle}" (${shortCommitId})`;
			} else {
				return `${shortCommitId}`;
			}
		})
		.join('\n');
	const branchCount = locks.map((lock) => lock.branchId).filter(unique).length;
	if (branchCount > 1) {
		return (
			'Warning, undefined behavior due to lock on multiple branches!\n\n' +
			'Locked because changes depend on:\n' +
			descriptions
		);
	}
	return 'Locked because changes depend on:\n' + descriptions;
}

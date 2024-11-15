import { HunkLock, type DetailedCommit } from './types';
import { uniqeByPropValues, unique } from '$lib/utils/filters';

export function getLockText(hunkLocks: HunkLock | HunkLock[], commits: DetailedCommit[]): string {
	if (!hunkLocks || commits === undefined) return 'Depends on a committed change';

	const locks = Array.isArray(hunkLocks) ? hunkLocks : [hunkLocks];

	const descriptions = locks
		.filter(uniqeByPropValues)
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

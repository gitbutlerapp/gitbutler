import { unique } from '$lib/utils/filters';
import type { Commit } from './types';

export function getLockText(commitId: string[] | string, commits: Commit[]): string {
	if (!commitId || commits === undefined) return 'Depends on a committed change';

	const lockedIds = typeof commitId == 'string' ? [commitId] : (commitId as string[]);

	const descriptions = lockedIds
		.filter(unique)
		.map((id) => {
			const commit = commits.find((commit) => commit.id == id);
			const shortCommitId = commit?.id.slice(0, 7);
			if (commit) {
				const shortTitle = commit.descriptionTitle?.slice(0, 35) + '...';
				return `"${shortTitle}" (${shortCommitId})`;
			} else {
				return `${shortCommitId}`;
			}
		})
		.join('\n');
	return 'Locked due to dependency on:\n' + descriptions;
}

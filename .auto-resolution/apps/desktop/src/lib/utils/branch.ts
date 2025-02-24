import type { BrandedId } from '@gitbutler/shared/utils/branding';

const PREFERRED_REMOTE = 'origin';
const BRANCH_SEPARATOR = '/';
const REF_REMOTES_PREFIX = 'refs/remotes/';
const REF_HEADS_PREFIX = 'refs/heads/';

export type BranchRef = BrandedId<'BranchRef'>;

export function createBranchRef(branchName: string, remote: string | undefined): BranchRef {
	if (remote) {
		return `${REF_REMOTES_PREFIX}${remote}${BRANCH_SEPARATOR}${branchName}` as BranchRef;
	}
	return `${REF_HEADS_PREFIX}${branchName}` as BranchRef;
}

/**
 * Get the branch name from a refname.
 *
 * If a remote is provided, the remote prefix will be removed.
 */
export function getBranchNameFromRef(ref: string, remote?: string): string | undefined {
	if (ref.startsWith(REF_REMOTES_PREFIX)) {
		ref = ref.replace(REF_REMOTES_PREFIX, '');
	}

	if (remote !== undefined) {
		const originPrefix = `${remote}${BRANCH_SEPARATOR}`;
		if (!ref.startsWith(originPrefix)) {
			throw new Error('Failed to parse branch name as reference');
		}
		ref = ref.replace(originPrefix, '');
	}

	return ref;
}

export function getBranchRemoteFromRef(ref: string): string | undefined {
	if (ref.startsWith(REF_REMOTES_PREFIX)) {
		ref = ref.slice(REF_REMOTES_PREFIX.length);
	}

	const parts = ref.split(BRANCH_SEPARATOR);
	return parts.length > 1 ? parts[0] : undefined;
}

const BRANCH_RANKING_EXACT: Record<string, number> = {
	'upstream/main': 100,
	'upstream/master': 100,
	'origin/main': 90,
	'origin/master': 90
};

const BRANCH_RANKING_ENDS_WITH: Record<string, number> = {
	'/master': 70,
	'/main': 70
};

function branchRank(branchName: string): number {
	const exactMatch = BRANCH_RANKING_EXACT[branchName];
	if (exactMatch !== undefined) {
		return exactMatch;
	}

	for (const [key, value] of Object.entries(BRANCH_RANKING_ENDS_WITH)) {
		if (branchName.endsWith(key)) {
			return value;
		}
	}

	return 10;
}

export function getBestBranch(branches: { name: string }[]): { name: string } | undefined {
	branches.sort((a, b) => branchRank(b.name) - branchRank(a.name));
	return branches[0];
}

export function getBestRemote(remotes: string[]): string | undefined {
	if (remotes.includes(PREFERRED_REMOTE)) {
		return PREFERRED_REMOTE;
	}

	return remotes[0];
}

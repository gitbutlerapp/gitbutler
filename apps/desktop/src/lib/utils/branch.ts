const PREFERRED_REMOTE = 'origin';
const BRANCH_SEPARATOR = '/';
const REF_REMOTES_PREFIX = 'refs/remotes/';

export function getBranchNameFromRef(ref: string): string | undefined {
	if (ref.startsWith(REF_REMOTES_PREFIX)) {
		ref = ref.slice(REF_REMOTES_PREFIX.length);
	}

	const parts = ref.split(BRANCH_SEPARATOR);
	return parts.length > 1 ? parts.slice(1).join(BRANCH_SEPARATOR) : ref;
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

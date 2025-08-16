const PREFERRED_REMOTE = 'origin';
const BRANCH_SEPARATOR = '/';
const REF_REMOTES_PREFIX = 'refs/remotes/';
const COMMON_REMOTE_NAMES = ['origin', 'upstream', 'fork', 'remote'] as const;

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

/**
 * Parses a branch name that may contain a remote prefix and returns the remote and branch name separately.
 * This handles cases where a remote branch is passed as "origin/feature/foo" instead of being split properly.
 *
 * @param branchName - The branch name, potentially with remote prefix (e.g., "origin/feature/foo")
 * @param providedRemote - The remote name if already provided separately
 * @returns Object with parsed remote and branch name
 */
export function parseBranchName(
	branchName: string,
	providedRemote?: string
): { remote?: string; branchName: string } {
	if (!branchName) {
		throw new Error('Branch name cannot be empty');
	}

	const parts = branchName.split(BRANCH_SEPARATOR);

	// No separator means it's a simple branch name
	if (parts.length <= 1) {
		return { remote: providedRemote, branchName };
	}

	const [firstPart, ...remainingParts] = parts;
	const branchNameWithoutPrefix = remainingParts.join(BRANCH_SEPARATOR);

	// Check if first part looks like a remote name
	const isKnownRemote = COMMON_REMOTE_NAMES.includes(firstPart as any);
	const matchesProvidedRemote = providedRemote === firstPart;

	if (isKnownRemote || matchesProvidedRemote) {
		return {
			remote: providedRemote || firstPart,
			branchName: branchNameWithoutPrefix
		};
	}

	// First part doesn't look like a remote, treat whole string as branch name
	return { remote: providedRemote, branchName };
}

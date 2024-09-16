import { entries, type UnknowObject } from './object';

const PREFERRED_REMOTE = 'origin';
const BRANCH_SEPARATOR = '/';

export function getBranchNameFromRef(ref: string): string | undefined {
	return ref.split(BRANCH_SEPARATOR).pop();
}

export function getBranchRemoteFromRef(ref: string): string | undefined {
	return ref.split(BRANCH_SEPARATOR)[0];
}

const BRANCH_RANKING_EXACT: UnknowObject<number> = {
	'upstream/main': 100,
	'upstream/master': 100,
	'origin/main': 90,
	'origin/master': 90
};

const BRANCH_RANKING_ENDS_WITH: UnknowObject<number> = {
	'/master': 70,
	'/main': 70
};

function branchRank(branchName: string): number {
	const exactMatch = BRANCH_RANKING_EXACT[branchName];
	if (exactMatch !== undefined) {
		return exactMatch;
	}

	for (const [key, value] of entries(BRANCH_RANKING_ENDS_WITH)) {
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

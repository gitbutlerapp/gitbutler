const PREFERRED_REMOTE = "origin";
const BRANCH_SEPARATOR = "/";
const REF_REMOTES_PREFIX = "refs/remotes/";
const REF_HEADS_PREFIX = "refs/heads/";

export function createBranchRef(branchName: string, remote: string | undefined): string {
	if (remote) {
		return `${REF_REMOTES_PREFIX}${remote}${BRANCH_SEPARATOR}${branchName}`;
	}
	return `${REF_HEADS_PREFIX}${branchName}`;
}

export function getBranchRemoteFromRef(ref: string): string | undefined {
	if (ref.startsWith(REF_REMOTES_PREFIX)) {
		ref = ref.slice(REF_REMOTES_PREFIX.length);
	}

	const parts = ref.split(BRANCH_SEPARATOR);
	return parts.length > 1 ? parts[0] : undefined;
}

export function getBestRemote(remotes: string[]): string | undefined {
	if (remotes.includes(PREFERRED_REMOTE)) {
		return PREFERRED_REMOTE;
	}

	return remotes[0];
}

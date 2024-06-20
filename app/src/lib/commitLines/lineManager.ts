import type { AnyCommit } from '$lib/vbranches/types';

interface CommitData {
    id: string,
    changeId: string
}

interface LineManagerProps {
	remoteCommits: CommitData[];
	localCommits: CommitData[];
	localAndRemoteCommits: CommitData[];
	integratedCommits: CommitData[];
}

function transformAnyCommit

/**
 * The Line Manager assumes that the groups of commits will be in the following order:
 * 1. Remote Commits (Commits you don't have in your branch)
 * 2. Local Commits (Commits that )
 */
export class LineManager {
	constructor({});
}

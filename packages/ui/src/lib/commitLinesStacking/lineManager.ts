import type { CommitData, LineData, CellType, Style } from '$lib/commitLinesStacking/types';

interface Commits {
	remoteCommits: CommitData[];
	localCommits: CommitData[];
	localAndRemoteCommits: CommitData[];
	integratedCommits: CommitData[];
}

// TODO: This can probably be simplified even further as we now only have 1 column (no forks) and we also don't
// split up commit colors, meaning that the top bar, the commit dot, and the bottom bar section are all always
// of the same color / type.
function generateLineData({
	remoteCommits,
	localCommits,
	localAndRemoteCommits,
	integratedCommits
}: Commits) {
	const remoteBranchGroups = mapToCommitLineGroupPair(remoteCommits);
	const localBranchGroups = mapToCommitLineGroupPair(localCommits);
	const localAndRemoteBranchGroups = mapToCommitLineGroupPair(localAndRemoteCommits);
	const integratedBranchGroups = mapToCommitLineGroupPair(integratedCommits);

	remoteBranchGroups.forEach(({ commit, line }) => {
		line.top.type = 'Upstream';
		line.bottom.type = 'Upstream';
		line.commitNode = { type: 'Upstream', commit };
	});

	let localCommitWithChangeIdFound = false;
	localBranchGroups.forEach(({ commit, line }) => {
		line.top.type = 'Local';
		line.bottom.type = 'Local';
		line.commitNode = { type: 'Local', commit };

		if (localCommitWithChangeIdFound) {
			// If a commit with a change ID has been found above this commit, use the leftStyle
			line.top.type = 'LocalShadow';
			line.bottom.type = 'LocalShadow';

			if (commit.relatedRemoteCommit) {
				line.commitNode = {
					type: 'LocalShadow',
					commit
				};
			}
		} else {
			if (commit.relatedRemoteCommit) {
				// For the first commit with a change ID found, only set the top if there are any remote commits
				if (remoteBranchGroups.length > 0) {
					line.top.type = 'LocalShadow';
				}

				line.commitNode = {
					type: 'LocalShadow',
					commit
				};
				line.bottom.type = 'LocalShadow';

				localCommitWithChangeIdFound = true;
			} else {
				// If there are any remote commits, continue the line
				if (remoteBranchGroups.length > 0) {
					line.top.type = 'LocalRemote';
					line.bottom.type = 'LocalRemote';
					line.commitNode.type = 'LocalRemote';
				}
			}
		}
	});

	localAndRemoteBranchGroups.forEach(({ commit, line }) => {
		line.top.type = 'LocalRemote';
		line.bottom.type = 'LocalRemote';
		line.commitNode = { type: 'LocalRemote', commit };
	});

	integratedBranchGroups.forEach(({ commit, line }) => {
		line.top.type = 'Integrated';
		line.bottom.type = 'Integrated';
		line.commitNode = { type: 'Integrated', commit };
	});

	const data = new Map<string, LineData>([
		...remoteBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...localBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...localAndRemoteBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...integratedBranchGroups.map(({ commit, line }) => [commit.id, line])
	] as [string, LineData][]);

	// Ensure bottom line is dashed
	[...data].reverse().find(([key, value]) => {
		if (!key.includes('-spacer')) {
			value.bottom.style = 'dashed';
			data.set(key, value);
			return true;
		}
		return false;
	});

	return { data };
}

function mapToCommitLineGroupPair(commits: CommitData[]) {
	if (commits.length === 0) return [];

	const groupings = commits.map((commit) => ({
		commit,
		line: {
			top: { type: 'Local' as CellType, style: 'solid' as Style },
			bottom: { type: 'Local' as CellType, style: 'solid' as Style },
			commitNode: { type: 'Local' as CellType, commit }
		}
	}));

	return groupings;
}

/**
 * The Line Manager assumes that the groups of commits will be in the following order:
 * 1. Remote Commits (Commits you don't have in your branch)
 * 2. Local Commits (Commits that you have changed locally)
 * 3. LocalAndRemote Commits (Commits that exist locally and on the remote and have the same hash)
 * 4. Integrated Commits (Commits that exist locally and perhaps on the remote that are in the trunk)
 */
export class LineManager {
	private data: Map<string, LineData>;

	constructor(commits: Commits) {
		const { data } = generateLineData(commits);
		this.data = data;
	}

	get(commitId: string) {
		if (!this.data.has(commitId)) {
			throw new Error(`Failed to find commit ${commitId} in line manager`);
		}

		return this.data.get(commitId)!;
	}
}

export class LineManagerFactory {
	build(commits: Commits) {
		return new LineManager(commits);
	}
}

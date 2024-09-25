import type { CommitData, LineData, CellType, Style } from '$lib/commitLinesStacking/types';

interface Commits {
	remoteCommits: CommitData[];
	localCommits: CommitData[];
	localAndRemoteCommits: CommitData[];
	integratedCommits: CommitData[];
}

function generateSameForkpoint({
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
		line.top.type = 'Remote';
		line.bottom.type = 'Remote';
		line.commitNode = { type: 'Remote', commit };

		// If there are local commits we want to fill in a local dashed line
		if (localBranchGroups.length > 0) {
			line.top.type = 'Local';
			line.bottom.type = 'Local';
			line.top.style = 'dashed';
			line.bottom.style = 'dashed';
		}
	});

	let localCommitWithChangeIdFound = false;
	localBranchGroups.forEach(({ commit, line }) => {
		line.top.type = 'Local';
		line.bottom.type = 'Local';
		line.commitNode = { type: 'Local', commit };

		// We need to use either remote or localAndRemote depending on what is above
		let leftType: CellType | undefined;

		if (remoteBranchGroups.length > 0) {
			leftType = 'Remote';
		} else {
			leftType = 'LocalShadow';
		}

		if (localCommitWithChangeIdFound) {
			// If a commit with a change ID has been found above this commit, use the leftStyle
			line.top.type = leftType;
			line.bottom.type = leftType;

			if (commit.relatedRemoteCommit) {
				line.commitNode = {
					type: 'Remote',
					commit: commit.relatedRemoteCommit
				};
			}
		} else {
			if (commit.relatedRemoteCommit) {
				// For the first commit with a change ID found, only set the top if there are any remote commits
				if (remoteBranchGroups.length > 0) {
					line.top.type = leftType;
				}

				line.commitNode = {
					type: 'Remote',
					commit: commit.relatedRemoteCommit
				};
				line.bottom.type = leftType;

				localCommitWithChangeIdFound = true;
			} else {
				// If there are any remote commits, continue the line
				if (remoteBranchGroups.length > 0) {
					line.top.type = 'Remote';
					line.bottom.type = 'Remote';
				}
			}
		}
	});

	localAndRemoteBranchGroups.forEach(({ commit, line }) => {
		line.top.type = 'LocalShadow';
		line.bottom.type = 'LocalShadow';
		line.commitNode = { type: 'LocalShadow', commit };
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

	return { data };
}

function generateDifferentForkpoint({
	remoteCommits,
	localCommits,
	localAndRemoteCommits,
	integratedCommits
}: Commits) {
	if (localAndRemoteCommits.length > 0) {
		throw new Error('There should never be local and remote commits with a different forkpoint');
	}

	const remoteBranchGroups = mapToCommitLineGroupPair(remoteCommits);
	const localBranchGroups = mapToCommitLineGroupPair(localCommits);
	const integratedBranchGroups = mapToCommitLineGroupPair(integratedCommits);

	remoteBranchGroups.forEach(({ commit, line }) => {
		line.top.type = 'Remote';
		line.bottom.type = 'Remote';
		line.commitNode = { type: 'Remote', commit };

		// If there are local commits further down, render a dashed line from the top of the list
		if (localBranchGroups.length > 0) {
			line.top.type = 'Local';
			line.bottom.type = 'Local';
			line.top.style = 'dashed';
			line.bottom.style = 'dashed';
		}
	});

	let localCommitWithChangeIdFound = false;
	localBranchGroups.forEach(({ commit, line }, index) => {
		// Make the top commit dashed
		if (index === 0) {
			line.top.style = 'dashed';
		}

		line.top.type = 'Local';
		line.bottom.type = 'Local';
		line.commitNode = { type: 'Local', commit };

		if (localCommitWithChangeIdFound) {
			// If a previous local commit with change ID was found, color with "shadow"
			line.top.type = 'LocalShadow';
			line.bottom.type = 'LocalShadow';

			if (commit.relatedRemoteCommit) {
				line.commitNode = {
					type: 'LocalShadow',
					commit: commit.relatedRemoteCommit
				};
			}
		} else {
			if (commit.relatedRemoteCommit) {
				// If the commit has a commit with a matching change ID, mark it in the shadow lane

				// Since this is the first, if there were any remote commits, we should inherit that color
				if (remoteBranchGroups.length > 0) {
					line.top.type = 'Remote';
				}

				line.commitNode = {
					type: 'Remote',
					commit: commit.relatedRemoteCommit
				};
				line.bottom.type = 'LocalShadow';

				localCommitWithChangeIdFound = true;
			} else {
				// Otherwise maintain the left color if it exists
				if (remoteBranchGroups.length > 0) {
					line.top.type = 'Remote';
					line.bottom.type = 'Remote';
				}
			}
		}
	});

	integratedBranchGroups.forEach(({ commit, line }, index) => {
		if (localBranchGroups.length === 0 && remoteBranchGroups.length === 0) {
			// If there are no local or remote branches, we want to have a single dashed line

			// Don't color in if its the first commit
			if (index !== 0) {
				line.top.type = 'Integrated';
				line.top.style = 'dashed';
			}

			line.bottom.type = 'Integrated';
			line.bottom.style = 'dashed';

			line.commitNode = { type: 'Integrated', commit };
		} else {
			// If we have local branches, maintain that color on the top half of the first commit
			if (index === 0) {
				line.top.type = localBranchGroups.at(-1)?.line.bottom.type || 'Local';
			} else {
				line.top.type = 'Integrated';
			}

			line.bottom.type = 'Integrated';
			line.commitNode = { type: 'Integrated', commit };

			if (localCommitWithChangeIdFound) {
				// If there is a commit with change id above, just use shadow style
				line.top.type = 'LocalShadow';
				line.bottom.type = 'LocalShadow';

				if (commit.relatedRemoteCommit) {
					line.commitNode = {
						type: 'LocalShadow',
						commit: commit.relatedRemoteCommit
					};
				}
			} else {
				if (commit.relatedRemoteCommit) {
					// If we have just found a commit with a shadow style, match the top style
					if (remoteBranchGroups.length > 0) {
						line.top.type = 'Remote';
					}

					line.commitNode = {
						type: 'Remote',
						commit: commit.relatedRemoteCommit
					};
					line.bottom.type = 'Remote';

					localCommitWithChangeIdFound = true;
				} else {
					// Otherwise style as remote if there are any
					if (remoteBranchGroups.length > 0) {
						line.top.type = 'Remote';
						line.bottom.type = 'Remote';
					}
				}
			}
		}
	});

	const data = new Map<string, LineData>([
		...remoteBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...localBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...integratedBranchGroups.map(({ commit, line }) => [commit.id, line])
	] as [string, LineData][]);

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

	constructor(commits: Commits, sameForkpoint: boolean) {
		// We should never have local and remote commits with a different forkpoint
		if (sameForkpoint || commits.localAndRemoteCommits.length > 0) {
			const { data } = generateSameForkpoint(commits);
			this.data = data;
		} else {
			const { data } = generateDifferentForkpoint(commits);
			this.data = data;
		}
	}

	get(commitId: string) {
		if (!this.data.has(commitId)) {
			throw new Error(`Failed to find commit ${commitId} in line manager`);
		}

		return this.data.get(commitId)!;
	}
}

export class LineManagerFactory {
	build(commits: Commits, sameForkpoint: boolean) {
		return new LineManager(commits, sameForkpoint);
	}
}

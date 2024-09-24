import type {
	CommitData,
	LineGroupData,
	LineData,
	Color,
	CellData,
	CommitNodeData
} from '$lib/commitLinesStacking/types';

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
	const LEFT_COLUMN_INDEX = 0;
	const RIGHT_COLUMN_INDEX = 1;

	const remoteBranchGroups = mapToCommitLineGroupPair(remoteCommits);
	const localBranchGroups = mapToCommitLineGroupPair(localCommits);
	console.log('generateSameForkpoint.localBranchGroups', localBranchGroups);
	const localAndRemoteBranchGroups = mapToCommitLineGroupPair(localAndRemoteCommits);
	const integratedBranchGroups = mapToCommitLineGroupPair(integratedCommits);

	const base = blankLineGroup();

	remoteBranchGroups.forEach(({ commit, line }, index) => {
		// We don't color in top half of the first remote commit
		if (index !== 0) {
			line.top.color = 'remote';
		}

		line.bottom.color = 'remote';
		line.commitNode = { type: 'Remote', commit };

		// If there are local commits we want to fill in a local dashed line
		if (localBranchGroups.length > 0) {
			line.top.color = 'local';
			line.bottom.color = 'local';
			line.top.style = 'dashed';
			line.bottom.style = 'dashed';
		}
	});

	let localCommitWithChangeIdFound = false;
	localBranchGroups.forEach(({ commit, line }, index) => {
		// The first local commit should have the top be dashed
		if (index === 0) {
			line.top.style = 'dashed';
		}

		line.top.color = 'local';
		line.bottom.color = 'local';
		line.commitNode = { type: 'Local', commit };

		// We need to use either remote or localAndRemote depending on what is above
		let leftStyle: Color | undefined;

		if (remoteBranchGroups.length > 0) {
			leftStyle = 'remote';
		} else {
			leftStyle = 'localAndRemote';
		}

		if (localCommitWithChangeIdFound) {
			// If a commit with a change ID has been found above this commit, use the leftStyle
			line.top.color = leftStyle;
			line.bottom.color = leftStyle;

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
					line.top.color = leftStyle;
				}

				line.commitNode = {
					type: 'Remote',
					commit: commit.relatedRemoteCommit
				};
				line.bottom.color = leftStyle;

				localCommitWithChangeIdFound = true;
			} else {
				// If there are any remote commits, continue the line
				if (remoteBranchGroups.length > 0) {
					line.top.color = 'remote';
					line.bottom.color = 'remote';
				}
			}
		}
	});

	localAndRemoteBranchGroups.forEach(({ commit, line }, index) => {
		if (index === 0) {
			// Copy the top color from any commits above for the first commit
			if (localBranchGroups.length > 0) {
				line.top.color = localBranchGroups.at(-1)!.line.bottom.color;
			} else if (remoteBranchGroups.length > 0) {
				lineGroup.lines[LEFT_COLUMN_INDEX].top.color = remoteBranchGroups.at(-1)!.line.bottom.color;
			}
		} else {
			line.top.color = 'localAndRemote';
		}
		line.bottom.color = 'localAndRemote';

		line.commitNode = { type: 'LocalShadow', commit };
	});

	integratedBranchGroups.forEach(({ commit, line }, index) => {
		if (index === 0) {
			// Copy the top color from any commits above for the first commit
			if (localAndRemoteBranchGroups.length > 0) {
				line.top.color = localAndRemoteBranchGroups.at(-1)!.line.bottom.color;
			} else if (localBranchGroups.length > 0) {
				line.top.color = localBranchGroups.at(-1)!.line.bottom.color;
			} else if (remoteBranchGroups.length > 0) {
				line.top.color = remoteBranchGroups.at(-1)!.line.bottom.color;
			}
		} else {
			line.top.color = 'integrated';
		}
		line.bottom.color = 'integrated';

		line.commitNode = { type: 'LocalShadow', commit };
	});

	// Set forkpoints
	if (localBranchGroups.length > 0) {
		if (localAndRemoteBranchGroups.length > 0) {
			localAndRemoteBranchGroups[0].line.top.color = 'local';
		} else if (integratedBranchGroups.length > 0) {
			integratedBranchGroups[0].line.top.color = 'local';
		}
	}

	// Remove padding column if unrequired
	// if (localBranchGroups.length === 0) {
	// 	remoteBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
	// 	localBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
	// 	localAndRemoteBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
	// 	integratedBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
	//
	// 	base.lines.pop();
	// }

	// Set base
	// base.lines[LEFT_COLUMN_INDEX].top.style = 'dashed';
	if (integratedBranchGroups.length > 0) {
		base.top.color = 'integrated';
	} else if (localAndRemoteBranchGroups.length > 0) {
		base.top.color = 'localAndRemote';
	} else if (localBranchGroups.length > 0) {
		base.top.color = 'local';
	} else if (remoteBranchGroups.length > 0) {
		base.top.color = 'remote';
	}

	const data = new Map<string, LineGroupData>([
		...remoteBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...localBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...localAndRemoteBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...integratedBranchGroups.map(({ commit, line }) => [commit.id, line])
	] as [string, LineGroupData][]);

	return { data, base };
}

function generateDifferentForkpoint({
	remoteCommits,
	localCommits,
	localAndRemoteCommits,
	integratedCommits
}: Commits) {
	const LEFT_COLUMN_INDEX = 0;
	const MIDDLE_COLUMN_INDEX = 1;
	const RIGHT_COLUMN_INDEX = 2;

	if (localAndRemoteCommits.length > 0) {
		throw new Error('There should never be local and remote commits with a different forkpoint');
	}

	const remoteBranchGroups = mapToCommitLineGroupPair(remoteCommits);
	const localBranchGroups = mapToCommitLineGroupPair(localCommits);
	const integratedBranchGroups = mapToCommitLineGroupPair(integratedCommits);

	const base = blankLineGroup();

	remoteBranchGroups.forEach(({ commit, line }, index) => {
		// Don't color top half if its the first commit of the list
		if (index !== 0) {
			line.top.color = 'remote';
		}

		line.bottom.color = 'remote';

		line.commitNode = { type: 'Remote', commit };

		// If there are local commits further down, render a dashed line from the top of the list
		if (localBranchGroups.length > 0) {
			line.top.color = 'local';
			line.bottom.color = 'local';
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

		line.top.color = 'local';
		line.bottom.color = 'local';
		line.commitNode = { type: 'Local', commit };

		if (localCommitWithChangeIdFound) {
			// If a previous local commit with change ID was found, color with "shadow"
			line.top.color = 'shadow';
			line.bottom.color = 'shadow';

			if (commit.relatedRemoteCommit) {
				line.commitNode = {
					type: 'Remote',
					commit: commit.relatedRemoteCommit
				};
			}
		} else {
			if (commit.relatedRemoteCommit) {
				// If the commit has a commit with a matching change ID, mark it in the shadow lane

				// Since this is the first, if there were any remote commits, we should inherit that color
				if (remoteBranchGroups.length > 0) {
					line.top.color = 'remote';
				}

				line.commitNode = {
					type: 'Remote',
					commit: commit.relatedRemoteCommit
				};
				line.bottom.color = 'shadow';

				localCommitWithChangeIdFound = true;
			} else {
				// Otherwise maintain the left color if it exists
				if (remoteBranchGroups.length > 0) {
					line.top.color = 'remote';
					line.bottom.color = 'remote';
				}
			}
		}
	});

	integratedBranchGroups.forEach(({ commit, line }, index) => {
		if (localBranchGroups.length === 0 && remoteBranchGroups.length === 0) {
			// If there are no local or remote branches, we want to have a single dashed line

			// Don't color in if its the first commit
			if (index !== 0) {
				line.top.color = 'integrated';
				line.top.style = 'dashed';
			}

			line.bottom.color = 'integrated';
			line.bottom.style = 'dashed';

			line.commitNode = { type: 'LocalShadow', commit };
		} else {
			// If we have local branches, maintain that color on the top half of the first commit
			if (index === 0) {
				line.top.color = localBranchGroups.at(-1)?.line.bottom.color || 'none';
			} else {
				line.top.color = 'integrated';
			}

			line.bottom.color = 'integrated';
			line.commitNode = { type: 'LocalShadow', commit };

			if (localCommitWithChangeIdFound) {
				// If there is a commit with change id above, just use shadow style
				line.top.color = 'shadow';
				line.bottom.color = 'shadow';

				if (commit.relatedRemoteCommit) {
					line.commitNode = {
						type: 'Remote',
						commit: commit.relatedRemoteCommit
					};
				}
			} else {
				if (commit.relatedRemoteCommit) {
					// If we have just found a commit with a shadow style, match the top style
					if (remoteBranchGroups.length > 0) {
						line.top.color = 'remote';
					}

					line.commitNode = {
						type: 'Remote',
						commit: commit.relatedRemoteCommit
					};
					line.bottom.color = 'shadow';

					localCommitWithChangeIdFound = true;
				} else {
					// Otherwise style as remote if there are any
					if (remoteBranchGroups.length > 0) {
						line.top.color = 'remote';
						line.bottom.color = 'remote';
					}
				}
			}
		}
	});

	function setLeftSideBase() {
		let color: Color | undefined;
		if (integratedBranchGroups.length > 0) {
			color = integratedBranchGroups.at(-1)!.line.bottom.color;
		} else if (localBranchGroups.length > 0) {
			color = localBranchGroups.at(-1)!.line.bottom.color;
		} else if (remoteBranchGroups.length > 0) {
			color = remoteBranchGroups.at(-1)!.line.bottom.color;
		} else {
			color = 'none';
		}

		base.top.color = color;
		base.bottom.color = color;
		base.top.style = 'dashed';
		base.bottom.style = 'dashed';
	}

	// Set base
	if (integratedBranchGroups.length > 0) {
		base.top.color = 'integrated';
		base.top.style = integratedBranchGroups.at(-1)!.line.bottom.style;

		setLeftSideBase();
	} else if (localBranchGroups.length > 0) {
		base.top.color = 'local';

		setLeftSideBase();
	} else if (remoteBranchGroups.length > 0) {
		base.top.color = 'remote';
		base.top.style = 'dashed';
	}

	function removeLeftMostColumn() {
		remoteBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.shift());
		localBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.shift());
		integratedBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.shift());
		base.lines.shift();
	}

	function removeRightMostColumn() {
		remoteBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
		localBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
		integratedBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
		base.lines.pop();
	}

	// Remove the left column if there is no ghost line
	// const hasGhostLine = [
	// 	...remoteBranchGroups,
	// 	...localBranchGroups,
	// 	...integratedBranchGroups
	// ].some(
	// 	({ lineGroup }) =>
	// 		lineGroup.lines[LEFT_COLUMN_INDEX].top.color !== 'none' ||
	// 		lineGroup.lines[LEFT_COLUMN_INDEX].bottom.color !== 'none'
	// );

	// if (!hasGhostLine) {
	// 	removeLeftMostColumn();
	// }

	// Remove the right two columns if there is only remote commits
	if (integratedBranchGroups.length === 0 && localBranchGroups.length === 0) {
		// removeRightMostColumn();
		// removeRightMostColumn();
	}

	// Remove one right column if there is only integrated with no local or remote commits
	if (
		integratedBranchGroups.length > 0 &&
		localBranchGroups.length === 0 &&
		remoteBranchGroups.length === 0
	) {
		// removeRightMostColumn();
	}

	const data = new Map<string, LineData>([
		...remoteBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...localBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...integratedBranchGroups.map(({ commit, line }) => [commit.id, line])
	] as [string, LineData][]);

	return { data, base };
}

function mapToCommitLineGroupPair(commits: CommitData[]): CommitNodeData[] {
	if (commits.length === 0) return [];
	const groupings = commits.map((commit) => ({
		commit,
		line: {
			top: { type: 'straight', color: 'none' },
			bottom: { type: 'straight', color: 'none' }
		}
	}));

	return groupings;
}

function blankLineGroup(): LineData {
	// const lines = Array(lineCount)
	// 	.fill(undefined)
	// 	.map(
	// 		(): LineData => ({
	// 			top: { type: 'straight', color: 'none' },
	// 			bottom: { type: 'straight', color: 'none' }
	// 		})
	// 	);
	//
	return {
		top: { color: 'none' },
		bottom: { color: 'none' }
	};
}

/**
 * The Line Manager assumes that the groups of commits will be in the following order:
 * 1. Remote Commits (Commits you don't have in your branch)
 * 2. Local Commits (Commits that you have changed locally)
 * 3. LocalAndRemote Commits (Commits that exist locally and on the remote and have the same hash)
 * 4. Integrated Commits (Commits that exist locally and perhaps on the remote that are in the trunk)
 */
export class LineManager {
	private data: Map<string, LineGroupData>;
	base: LineGroupData;

	constructor(commits: Commits, sameForkpoint: boolean) {
		// We should never have local and remote commits with a different forkpoint
		if (sameForkpoint || commits.localAndRemoteCommits.length > 0) {
			console.log('SAME_FORK_POINT.COMMITS', commits);
			const { data, base } = generateSameForkpoint(commits);
			this.data = data;
			this.base = base;
		} else {
			const { data, base } = generateDifferentForkpoint(commits);
			this.data = data;
			this.base = base;
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

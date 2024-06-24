import type { CommitData, LineGroup, Line, Style } from '$lib/commitLines/types';

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

	const remoteBranchGroups = mapToCommitLineGroupPair(remoteCommits, 3);
	const localBranchGroups = mapToCommitLineGroupPair(localCommits, 3);
	const localAndRemoteBranchGroups = mapToCommitLineGroupPair(localAndRemoteCommits, 3);
	const integratedBranchGroups = mapToCommitLineGroupPair(integratedCommits, 3);

	const base = blankLineGroup(3);

	remoteBranchGroups.forEach(({ commit, lineGroup }, index) => {
		if (index !== 0) {
			lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'remote';
		}
		lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'remote';

		lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = { type: 'large', commit };

		if (localBranchGroups.length > 0) {
			lineGroup.lines[RIGHT_COLUMN_INDEX].top.style = 'localDashed';
			lineGroup.lines[RIGHT_COLUMN_INDEX].bottom.style = 'localDashed';
		}
	});

	let localCommitWithChangeIdFound = false;
	localBranchGroups.forEach(({ commit, lineGroup }, index) => {
		if (index === 0) {
			lineGroup.lines[RIGHT_COLUMN_INDEX].top.style = 'localDashed';
		} else {
			lineGroup.lines[RIGHT_COLUMN_INDEX].top.style = 'local';
		}
		lineGroup.lines[RIGHT_COLUMN_INDEX].bottom.style = 'local';
		lineGroup.lines[RIGHT_COLUMN_INDEX].commitNode = { type: 'large', commit };

		let leftStyle: Style | undefined;

		if (remoteBranchGroups.length > 0) {
			leftStyle = 'remote';
		} else {
			leftStyle = 'localAndRemote';
		}

		if (localCommitWithChangeIdFound) {
			lineGroup.lines[LEFT_COLUMN_INDEX].top.style = leftStyle;
			lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = leftStyle;

			if (commit.relatedRemoteCommit) {
				lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = {
					type: 'small',
					commit: commit.relatedRemoteCommit
				};
			}
		} else {
			if (commit.relatedRemoteCommit) {
				if (remoteBranchGroups.length > 0) {
					lineGroup.lines[LEFT_COLUMN_INDEX].top.style = leftStyle;
				}

				lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = {
					type: 'small',
					commit: commit.relatedRemoteCommit
				};
				lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = leftStyle;

				localCommitWithChangeIdFound = true;
			} else {
				if (remoteBranchGroups.length > 0) {
					lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'remote';
					lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'remote';
				}
			}
		}
	});

	localAndRemoteBranchGroups.forEach(({ commit, lineGroup }, index) => {
		if (index === 0) {
			if (localBranchGroups.length > 0) {
				lineGroup.lines[LEFT_COLUMN_INDEX].top.style =
					localBranchGroups.at(-1)!.lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style;
			} else if (remoteBranchGroups.length > 0) {
				lineGroup.lines[LEFT_COLUMN_INDEX].top.style =
					remoteBranchGroups.at(-1)!.lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style;
			}
		} else {
			lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'localAndRemote';
		}
		lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'localAndRemote';

		lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = { type: 'large', commit };
	});

	integratedBranchGroups.forEach(({ commit, lineGroup }, index) => {
		if (index === 0) {
			if (localAndRemoteBranchGroups.length > 0) {
				lineGroup.lines[LEFT_COLUMN_INDEX].top.style =
					localAndRemoteBranchGroups.at(-1)!.lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style;
			} else if (localBranchGroups.length > 0) {
				lineGroup.lines[LEFT_COLUMN_INDEX].top.style =
					localBranchGroups.at(-1)!.lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style;
			} else if (remoteBranchGroups.length > 0) {
				lineGroup.lines[LEFT_COLUMN_INDEX].top.style =
					remoteBranchGroups.at(-1)!.lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style;
			}
		} else {
			lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'integrated';
		}
		lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'integrated';

		lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = { type: 'large', commit };
	});

	// Set forkpoints
	if (localBranchGroups.length > 0) {
		if (integratedBranchGroups.length === 0 && localAndRemoteBranchGroups.length === 0) {
			localBranchGroups.at(-1)!.lineGroup.lines[RIGHT_COLUMN_INDEX].bottom.type = 'fork';
		} else if (localAndRemoteBranchGroups.length > 0) {
			localAndRemoteBranchGroups[0].lineGroup.lines[RIGHT_COLUMN_INDEX].top.type = 'fork';
			localAndRemoteBranchGroups[0].lineGroup.lines[RIGHT_COLUMN_INDEX].top.style = 'local';
		} else if (integratedBranchGroups.length > 0) {
			integratedBranchGroups[0].lineGroup.lines[RIGHT_COLUMN_INDEX].top.type = 'fork';
			integratedBranchGroups[0].lineGroup.lines[RIGHT_COLUMN_INDEX].top.style = 'local';
		}
	}

	// Remove padding column if unrequired
	if (localBranchGroups.length === 0) {
		remoteBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
		localBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
		localAndRemoteBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());
		integratedBranchGroups.forEach(({ lineGroup }) => lineGroup.lines.pop());

		base.lines.pop();
	}

	// Set base
	base.lines[LEFT_COLUMN_INDEX].baseNode = {};
	if (integratedBranchGroups.length > 0) {
		base.lines[LEFT_COLUMN_INDEX].top.style = 'integrated';
	} else if (localAndRemoteBranchGroups.length > 0) {
		base.lines[LEFT_COLUMN_INDEX].top.style = 'localAndRemote';
	} else if (localBranchGroups.length > 0) {
		base.lines[LEFT_COLUMN_INDEX].top.style = 'local';
	} else if (remoteBranchGroups.length > 0) {
		base.lines[LEFT_COLUMN_INDEX].top.style = 'remote';
	} else {
		base.lines[LEFT_COLUMN_INDEX].baseNode = undefined;
	}

	const data = new Map<string, LineGroup>([
		...remoteBranchGroups.map(({ commit, lineGroup }) => [commit.id, lineGroup]),
		...localBranchGroups.map(({ commit, lineGroup }) => [commit.id, lineGroup]),
		...localAndRemoteBranchGroups.map(({ commit, lineGroup }) => [commit.id, lineGroup]),
		...integratedBranchGroups.map(({ commit, lineGroup }) => [commit.id, lineGroup])
	] as [string, LineGroup][]);

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

	const remoteBranchGroups = mapToCommitLineGroupPair(remoteCommits, 4);
	const localBranchGroups = mapToCommitLineGroupPair(localCommits, 4);
	const integratedBranchGroups = mapToCommitLineGroupPair(integratedCommits, 4);

	const base = blankLineGroup(4);

	remoteBranchGroups.forEach(({ commit, lineGroup }, index) => {
		if (index !== 0) {
			lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'remote';
		}
		lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'remote';

		lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = { type: 'large', commit };

		if (localBranchGroups.length > 0) {
			lineGroup.lines[RIGHT_COLUMN_INDEX].top.style = 'localDashed';
			lineGroup.lines[RIGHT_COLUMN_INDEX].bottom.style = 'localDashed';
		}
	});

	let localCommitWithChangeIdFound = false;
	localBranchGroups.forEach(({ commit, lineGroup }, index) => {
		if (index === 0) {
			lineGroup.lines[RIGHT_COLUMN_INDEX].top.style = 'localDashed';
		} else {
			lineGroup.lines[RIGHT_COLUMN_INDEX].top.style = 'local';
		}
		lineGroup.lines[RIGHT_COLUMN_INDEX].bottom.style = 'local';
		lineGroup.lines[RIGHT_COLUMN_INDEX].commitNode = { type: 'large', commit };

		if (localCommitWithChangeIdFound) {
			lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'shadow';
			lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'shadow';

			if (commit.relatedRemoteCommit) {
				lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = {
					type: 'small',
					commit: commit.relatedRemoteCommit
				};
			}
		} else {
			if (commit.relatedRemoteCommit) {
				if (remoteBranchGroups.length > 0) {
					lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'remote';
				}

				lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = {
					type: 'small',
					commit: commit.relatedRemoteCommit
				};
				lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'shadow';

				localCommitWithChangeIdFound = true;
			} else {
				if (remoteBranchGroups.length > 0) {
					lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'remote';
					lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'remote';
				}
			}
		}
	});

	integratedBranchGroups.forEach(({ commit, lineGroup }, index) => {
		if (index === 0) {
			lineGroup.lines[RIGHT_COLUMN_INDEX].top.style =
				localBranchGroups.at(-1)?.lineGroup.lines[RIGHT_COLUMN_INDEX].bottom.style || 'none';
		} else {
			lineGroup.lines[RIGHT_COLUMN_INDEX].top.style = 'integrated';
		}
		lineGroup.lines[RIGHT_COLUMN_INDEX].bottom.style = 'integrated';
		lineGroup.lines[RIGHT_COLUMN_INDEX].commitNode = { type: 'large', commit };

		if (localCommitWithChangeIdFound) {
			lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'shadow';
			lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'shadow';

			if (commit.relatedRemoteCommit) {
				lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = {
					type: 'small',
					commit: commit.relatedRemoteCommit
				};
			}
		} else {
			if (commit.relatedRemoteCommit) {
				if (remoteBranchGroups.length > 0) {
					lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'remote';
				}

				lineGroup.lines[LEFT_COLUMN_INDEX].commitNode = {
					type: 'small',
					commit: commit.relatedRemoteCommit
				};
				lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'shadow';

				localCommitWithChangeIdFound = true;
			} else {
				if (remoteBranchGroups.length > 0) {
					lineGroup.lines[LEFT_COLUMN_INDEX].top.style = 'remote';
					lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style = 'remote';
				}
			}
		}
	});

	if (integratedBranchGroups.length > 0) {
		integratedBranchGroups.at(-1)!.lineGroup.lines[RIGHT_COLUMN_INDEX].bottom.type = 'fork';
	} else if (localBranchGroups.length > 0) {
		localBranchGroups.at(-1)!.lineGroup.lines[RIGHT_COLUMN_INDEX].bottom.type = 'fork';
	}

	function setLeftSideBase() {
		let style: Style | undefined;
		if (integratedBranchGroups.length > 0) {
			style = integratedBranchGroups.at(-1)!.lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style;
		} else if (localBranchGroups.length > 0) {
			style = localBranchGroups.at(-1)!.lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style;
		} else if (remoteBranchGroups.length > 0) {
			style = remoteBranchGroups.at(-1)!.lineGroup.lines[LEFT_COLUMN_INDEX].bottom.style;
		} else {
			style = 'none';
		}

		base.lines[LEFT_COLUMN_INDEX].top.style = style;
		base.lines[LEFT_COLUMN_INDEX].bottom.style = style;
	}

	// Set base
	if (integratedBranchGroups.length > 0) {
		base.lines[MIDDLE_COLUMN_INDEX].top.style = 'integrated';
		base.lines[MIDDLE_COLUMN_INDEX].baseNode = {};

		setLeftSideBase();
	} else if (localBranchGroups.length > 0) {
		base.lines[MIDDLE_COLUMN_INDEX].top.style = 'local';
		base.lines[MIDDLE_COLUMN_INDEX].baseNode = {};

		setLeftSideBase();
	} else if (remoteBranchGroups.length > 0) {
		base.lines[LEFT_COLUMN_INDEX].top.style = 'remote';
		base.lines[LEFT_COLUMN_INDEX].baseNode = {};
	}

	const data = new Map<string, LineGroup>([
		...remoteBranchGroups.map(({ commit, lineGroup }) => [commit.id, lineGroup]),
		...localBranchGroups.map(({ commit, lineGroup }) => [commit.id, lineGroup]),
		...integratedBranchGroups.map(({ commit, lineGroup }) => [commit.id, lineGroup])
	] as [string, LineGroup][]);

	return { data, base };
}

function mapToCommitLineGroupPair(commits: CommitData[], groupSize: number) {
	const groupings = commits.map((commit) => ({
		commit,
		lineGroup: blankLineGroup(groupSize)
	}));

	if (groupings.length > 0) {
		groupings[0].lineGroup.lines.forEach((line) => (line.tallerTop = true));
	}

	return groupings;
}

function blankLineGroup(lineCount: number): LineGroup {
	const lines = Array(lineCount)
		.fill(undefined)
		.map(
			(): Line => ({
				top: { type: 'straight', style: 'none' },
				bottom: { type: 'straight', style: 'none' }
			})
		);

	return {
		lines
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
	private data: Map<string, LineGroup>;
	base: LineGroup;

	constructor(commits: Commits, sameForkpoint: boolean) {
		// We should never have local and remote commits with a different forkpoint
		if (sameForkpoint || commits.localAndRemoteCommits.length > 0) {
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

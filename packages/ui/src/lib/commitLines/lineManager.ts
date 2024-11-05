import type { CommitData, LineData, CellType, Style } from '$lib/commitLines/types';

export enum LineSpacer {
	Remote = 'remote-spacer',
	Local = 'local-spacer',
	LocalAndRemote = 'local-and-remote-spacer'
}

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
		line.top.type = 'remote';
		line.bottom.type = 'remote';
		line.commitNode = { type: 'remote', commit };
	});

	localBranchGroups.forEach(({ commit, line }) => {
		line.top.type = 'local';
		line.bottom.type = 'local';
		line.commitNode = { type: 'local', commit };
	});

	localAndRemoteBranchGroups.forEach(({ commit, line }) => {
		if (line.commitNode.commit.remoteCommitId !== line.commitNode.commit.id) {
			line.commitNode = { type: 'localAndShadow', commit };
			line.top.type = 'localAndShadow';
			line.bottom.type = 'localAndShadow';
		} else {
			line.commitNode = { type: 'localAndRemote', commit };
			line.top.type = 'localAndRemote';
			line.bottom.type = 'localAndRemote';
		}
	});

	integratedBranchGroups.forEach(({ commit, line }) => {
		line.top.type = 'integrated';
		line.bottom.type = 'integrated';
		line.commitNode = { type: 'integrated', commit };
	});

	const data = new Map<string, LineData>([
		...remoteBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...localBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...localAndRemoteBranchGroups.map(({ commit, line }) => [commit.id, line]),
		...integratedBranchGroups.map(({ commit, line }) => [commit.id, line])
	] as [string, LineData][]);

	return { data };
}

function mapToCommitLineGroupPair(commits: CommitData[]) {
	if (commits.length === 0) return [];

	const groupings = commits.map((commit) => ({
		commit,
		line: {
			top: { type: 'local' as CellType, style: 'solid' as Style },
			bottom: { type: 'local' as CellType, style: 'solid' as Style },
			commitNode: { type: 'local' as CellType, commit }
		}
	}));

	return groupings;
}

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

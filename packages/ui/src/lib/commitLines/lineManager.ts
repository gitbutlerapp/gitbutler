import type { CommitData, CommitNodeData, CellType } from '$lib/commitLines/types';

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

function generateLineData({
	remoteCommits,
	localCommits,
	localAndRemoteCommits,
	integratedCommits
}: Commits) {
	const commitGroups = [
		{ commits: remoteCommits, type: 'remote' as CellType },
		{ commits: localCommits, type: 'local' as CellType },
		{ commits: integratedCommits, type: 'integrated' as CellType }
	];

	const localAndRemoteGroups = localAndRemoteCommits.map((commit) => {
		const type: CellType =
			commit.remoteCommitId === commit.id ? 'localAndRemote' : 'localAndShadow';
		return { commit, commitNode: { type, commit } };
	});

	const groupedData = commitGroups.flatMap(({ commits, type }) =>
		commits.map((commit) => ({
			commit,
			commitNode: { type, commit }
		}))
	);

	const allCommitData = [...groupedData, ...localAndRemoteGroups];

	const data = new Map<string, CommitNodeData>(
		allCommitData.map(({ commit, commitNode }) => [commit.id, commitNode])
	);

	return { data };
}

export class LineManager {
	private data: Map<string, CommitNodeData>;

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

import type { CommitData, CommitNodeData, CellType } from '$components/commitLines/types';

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
	const commitGroups: { commits: CommitData[]; type: CellType }[] = [
		{ commits: remoteCommits, type: 'Remote' },
		{ commits: localCommits, type: 'LocalOnly' },
		{ commits: localAndRemoteCommits, type: 'LocalAndRemote' },
		{ commits: integratedCommits, type: 'Integrated' }
	];

	const groupedData = commitGroups.flatMap(({ commits, type }) =>
		commits.map((commit) => ({
			commit,
			commitNode: { type, commit }
		}))
	);

	const data = new Map<string, CommitNodeData>(
		groupedData.map(({ commit, commitNode }) => [commit.id, commitNode])
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

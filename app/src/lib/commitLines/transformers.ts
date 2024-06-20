import type { CommitData, Author } from '$lib/commitLines/types';
import type { AnyCommit } from '$lib/vbranches/types';

export function transformAnyCommit(anyCommit: AnyCommit): CommitData {
	const output: CommitData = pullCommitDetails(anyCommit);

	if (anyCommit.relatedTo) {
		output.relatedRemoteCommit = pullCommitDetails(anyCommit.relatedTo);
	}

	return output;
}

function pullCommitDetails(anyCommit: AnyCommit): Omit<CommitData, 'relatedRemoteCommit'> {
	const author: Author = {
		email: anyCommit.author.email,
		gravatarUrl: anyCommit.author.gravatarUrl
	};

	return {
		id: anyCommit.id,
		author
	};
}

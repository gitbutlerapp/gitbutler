import type { Commit } from '$lib/vbranches/types';

/**
 * This class is used to determine how far a commit has been drag and dropped.
 *
 * We expect the dropzones to be in the following order:
 *
 * ```
 * const indexer = new ReorderDropzoneIndexer(commits);
 *
 * <ReorderDropzone index={indexer.topDropzoneIndex} />
 * <Commit id={commits[0].id} />
 * <ReorderDropzone index={indexer.dropzoneIndexBelowCommit(commits[0].id)} />
 * <Commit id={commits[1].id} />
 * <ReorderDropzone index={indexer.dropzoneIndexBelowCommit(commits[1].id)} />
 * ```
 */
export class ReorderDropzoneIndexer {
	private dropzoneIndexes = new Map<string, number>();
	private commitIndexes = new Map<string, number>();

	constructor(commits: Commit[]) {
		this.dropzoneIndexes.set('top', 0);

		commits.forEach((commit, index) => {
			this.dropzoneIndexes.set(commit.id, index + 1);
			this.commitIndexes.set(commit.id, index);
		});
	}

	get topDropzoneIndex() {
		return this.dropzoneIndexes.get('top') ?? 0;
	}

	dropzoneIndexBelowCommit(commitId: string) {
		const index = this.dropzoneIndexes.get(commitId);

		if (index == undefined) {
			throw new Error(`Commit ${commitId} not found in dropzoneIndexes`);
		}

		return index;
	}

	commitIndex(commitId: string) {
		const index = this.commitIndexes.get(commitId);

		if (index == undefined) {
			throw new Error(`Commit ${commitId} not found in commitIndexes`);
		}

		return index;
	}

	/**
	 * A negative offset means the commit has been dragged up, and a positive offset means the commit has been dragged down.
	 */
	dropzoneCommitOffset(dropzoneIndex: number, commitId: string) {
		const commitIndex = this.commitIndexes.get(commitId);

		if (commitIndex == undefined) {
			throw new Error(`Commit ${commitId} not found in commitIndexes`);
		}

		const offset = dropzoneIndex - commitIndex;

		if (offset > 0) {
			return offset - 1;
		} else {
			return offset;
		}
	}
}

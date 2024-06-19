import { DraggableCommit } from '$lib/dragging/draggables';
import type { BranchController } from '$lib/vbranches/branchController';
import type { Branch, Commit } from '$lib/vbranches/types';

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

// Exported for type access only
export class ReorderDropzone {
	constructor(
		private branchController: BranchController,
		private branch: Branch,
		private index: number,
		private indexer: ReorderDropzoneManager
	) {}

	accepts(data: any) {
		if (!(data instanceof DraggableCommit)) return false;
		if (data.branchId !== this.branch.id) return false;
		if (this.indexer.dropzoneCommitOffset(this.index, data.commit.id) === 0) return false;

		return true;
	}

	onDrop(data: any) {
		if (!(data instanceof DraggableCommit)) return;
		if (data.branchId !== this.branch.id) return;

		const offset = this.indexer.dropzoneCommitOffset(this.index, data.commit.id);
		this.branchController.reorderCommit(this.branch.id, data.commit.id, offset);
	}
}

export class ReorderDropzoneManager {
	private dropzoneIndexes = new Map<string, number>();
	private commitIndexes = new Map<string, number>();

	constructor(
		private branchController: BranchController,
		private branch: Branch,
		commits: Commit[]
	) {
		this.dropzoneIndexes.set('top', 0);

		commits.forEach((commit, index) => {
			this.dropzoneIndexes.set(commit.id, index + 1);
			this.commitIndexes.set(commit.id, index);
		});
	}

	get topDropzone() {
		const index = this.dropzoneIndexes.get('top') ?? 0;

		return new ReorderDropzone(this.branchController, this.branch, index, this);
	}

	dropzoneBelowCommit(commitId: string) {
		const index = this.dropzoneIndexes.get(commitId);

		if (index === undefined) {
			throw new Error(`Commit ${commitId} not found in dropzoneIndexes`);
		}

		return new ReorderDropzone(this.branchController, this.branch, index, this);
	}

	commitIndex(commitId: string) {
		const index = this.commitIndexes.get(commitId);

		if (index === undefined) {
			throw new Error(`Commit ${commitId} not found in commitIndexes`);
		}

		return index;
	}

	/**
	 * A negative offset means the commit has been dragged up, and a positive offset means the commit has been dragged down.
	 */
	dropzoneCommitOffset(dropzoneIndex: number, commitId: string) {
		const commitIndex = this.commitIndexes.get(commitId);

		if (commitIndex === undefined) {
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

export class ReorderDropzoneManagerFactory {
	constructor(private branchController: BranchController) {}

	build(branch: Branch, commits: Commit[]) {
		return new ReorderDropzoneManager(this.branchController, branch, commits);
	}
}

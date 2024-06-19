import { DraggableCommit } from '$lib/dragging/draggables';
import type { BranchController } from '$lib/vbranches/branchController';
import type { Branch, Commit } from '$lib/vbranches/types';

// Exported for type access only
export class ReorderDropzone {
	constructor(
		private branchController: BranchController,
		private branch: Branch,
		private entry: Entry
	) {}

	accepts(data: any) {
		if (!(data instanceof DraggableCommit)) return false;
		if (data.branchId !== this.branch.id) return false;
		if (this.entry.distanceToOtherCommit(data.commit.id) === 0) return false;

		return true;
	}

	onDrop(data: any) {
		if (!(data instanceof DraggableCommit)) return;
		if (data.branchId !== this.branch.id) return;

		const offset = this.entry.distanceToOtherCommit(data.commit.id);
		this.branchController.reorderCommit(this.branch.id, data.commit.id, offset);
	}
}

export class ReorderDropzoneManager {
	private indexer: Indexer;

	constructor(
		private branchController: BranchController,
		private branch: Branch,
		commits: Commit[]
	) {
		this.indexer = new Indexer(commits);
	}

	get topDropzone() {
		const entry = this.indexer.get('top');

		return new ReorderDropzone(this.branchController, this.branch, entry);
	}

	dropzoneBelowCommit(commitId: string) {
		const entry = this.indexer.get(commitId);

		return new ReorderDropzone(this.branchController, this.branch, entry);
	}
}

export class ReorderDropzoneManagerFactory {
	constructor(private branchController: BranchController) {}

	build(branch: Branch, commits: Commit[]) {
		return new ReorderDropzoneManager(this.branchController, branch, commits);
	}
}

// Private classes used to calculate distances between commtis
class Indexer {
	private dropzoneIndexes = new Map<string, number>();
	private commitIndexes = new Map<string, number>();

	constructor(commits: Commit[]) {
		this.dropzoneIndexes.set('top', 0);

		commits.forEach((commit, index) => {
			this.dropzoneIndexes.set(commit.id, index + 1);
			this.commitIndexes.set(commit.id, index);
		});
	}

	get(key: string) {
		const index = this.getIndex(key);

		return new Entry(this.commitIndexes, index);
	}

	private getIndex(key: string) {
		if (key === 'top') {
			return this.dropzoneIndexes.get(key) ?? 0;
		} else {
			const index = this.dropzoneIndexes.get(key);

			if (index === undefined) {
				throw new Error(`Commit ${key} not found in dropzoneIndexes`);
			}

			return index;
		}
	}
}

class Entry {
	constructor(
		private commitIndexes: Map<string, number>,
		private index: number
	) {}

	/**
	 * A negative offset means the commit has been dragged up, and a positive offset means the commit has been dragged down.
	 */
	distanceToOtherCommit(commitId: string) {
		const commitIndex = this.commitIndex(commitId);

		const offset = this.index - commitIndex;

		if (offset > 0) {
			return offset - 1;
		} else {
			return offset;
		}
	}

	private commitIndex(commitId: string) {
		const index = this.commitIndexes.get(commitId);

		if (index === undefined) {
			throw new Error(`Commit ${commitId} not found in commitIndexes`);
		}

		return index;
	}
}

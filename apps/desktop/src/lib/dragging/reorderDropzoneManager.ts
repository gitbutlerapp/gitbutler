import { DraggableCommit } from '$lib/dragging/draggables';
import type { BranchController } from '$lib/vbranches/branchController';
import type { VirtualBranch, DetailedCommit, PatchSeries } from '$lib/vbranches/types';

// Exported for type access only
export class ReorderDropzone {
	constructor(
		private branchController: BranchController,
		private branch: VirtualBranch,
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

		console.log('REORDERING.COMMIT', {
			branchId: this.branch.id,
			commitId: data.commit.id,
			offset
		});

		this.branchController.reorderCommit(this.branch.id, data.commit.id, offset);
	}
}

export class ReorderDropzoneManager {
	private indexer: Indexer;
	private branchController: BranchController;
	private branch: VirtualBranch;

	constructor({
		branchController,
		branch,
		commitIds
	}: {
		branchController: BranchController;
		branch: VirtualBranch;
		commitIds: string[];
	}) {
		this.branchController = branchController;
		this.branch = branch;

		this.indexer = new Indexer(commitIds);
	}

	topDropzone(key: string) {
		const entry = this.indexer.get(key);

		return new ReorderDropzone(this.branchController, this.branch, entry);
	}

	dropzoneBelowCommit(commitId: string) {
		const entry = this.indexer.get(commitId);

		return new ReorderDropzone(this.branchController, this.branch, entry);
	}
}

export class ReorderDropzoneManagerFactory {
	constructor(private branchController: BranchController) {}

	build({ branch, commitIds }: { branch: VirtualBranch; commitIds: string[] }) {
		return new ReorderDropzoneManager({
			branchController: this.branchController,
			branch,
			commitIds
		});
	}
}

// Private classes used to calculate distances between commits
class Indexer {
	// private dropzoneIndexes = new Map<string, number>();
	private dropzoneIndexes = new Map<string, number>();

	constructor(commitIds: string[]) {
		let computedPatchIndex = 0;

		commitIds.map((patchId: string) => {
			computedPatchIndex += 1;
			// this.dropzoneIndexes.set(patchId, computedPatchIndex + 1); // + 1); // (seriesIndex + 1));
			this.dropzoneIndexes.set(patchId, computedPatchIndex);
		});

		// console.log('Indexer.dropzoneIndexes', this.dropzoneIndexes);
		console.log('Indexer.dropzoneIndexes', this.dropzoneIndexes);
	}

	get(key: string) {
		const index = this.getIndex(key);

		return new Entry(this.dropzoneIndexes, index ?? 0);
	}

	private getIndex(key: string) {
		// Not 0, because 'top' will always be in the map
		if (this.dropzoneIndexes.size === 1) {
			return 0;
		}

		if (key === 'top') {
			return this.dropzoneIndexes.get(key) ?? 0;
		} else {
			const index = this.dropzoneIndexes.get(key);

			// TODO: Improve reactivity of dropzoneIndexes
			// Handle integrated state and dont error
			// if (index === undefined) {
			// 	throw new Error(`Commit ${key} not found in dropzoneIndexes`);
			// }

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
		if (commitIndex === undefined) return 0;

		const offset = this.index - commitIndex;
		console.log('distanceToOtherCommit', { offset, targetIndex: this.index, myIndex: commitIndex });

		return offset;
		// if (offset > 0) {
		// 	return offset - 1;
		// } else {
		// 	return offset;
		// }
	}

	private commitIndex(commitId: string) {
		const index = this.commitIndexes.get(commitId);

		// TODO: Handle updated commitIds after rebasing in `commitIndexes`
		// Reordering works, but it throws errors for old commitIds that it can't find
		// anymore after rebasing, for example.
		// if (index === undefined) {
		// 	throw new Error(`Commit ${commitId} not found in commitIndexes`);
		// }

		return index;
	}
}

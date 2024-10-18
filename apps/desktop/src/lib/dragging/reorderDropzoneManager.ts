import { DraggableCommit } from '$lib/dragging/draggables';
import type { BranchController } from '$lib/vbranches/branchController';
// import type { VirtualBranch, DetailedCommit, PatchSeries } from '$lib/vbranches/types';

// Exported for type access only
export class ReorderDropzone {
	constructor(
		private branchController: BranchController,
		private entry: Entry
	) {}

	accepts(data: any) {
		if (!data) return false;
		console.log('accepts.data', data);
		if (!(data instanceof DraggableCommit)) return false;
		if (this.entry.distanceToOtherCommit(data.commit.id) === 0) return false;

		return true;
	}

	onDrop(data: any) {
		if (!(data instanceof DraggableCommit)) return;
		const offset = this.entry.distanceToOtherCommit(data.commit.id);

		console.log('REORDERING.COMMIT', {
			commitId: data.commit.id,
			offset
		});

		// TODO: Can we get a branchId (seriesId) onto the PatchSeries?
		// The branchId is always the same for a whole lane/stack, so when dropping,
		// even if you have hte index correclty, you can't know really which series
		// your supposed to be landing in
		this.branchController.reorderCommit(data.commit?.branchId, data.commit?.id, offset);
	}
}

export class ReorderDropzoneManager {
	private indexer: Indexer;
	private branchController: BranchController;

	constructor({
		branchController,
		commits
	}: {
		branchController: BranchController;
		commits: string[];
	}) {
		this.branchController = branchController;
		this.indexer = new Indexer(commits);
	}

	dropzone(key: string) {
		const entry = this.indexer.get(key);

		return new ReorderDropzone(this.branchController, entry);
	}
}

export class ReorderDropzoneManagerFactory {
	constructor(private branchController: BranchController) {}

	build(commits: string[]) {
		return new ReorderDropzoneManager({
			branchController: this.branchController,
			commits
		});
	}
}

// Private classes used to calculate distances between commits
class Indexer {
	private dropzoneIndexes = new Map<string, number>();

	constructor(commits: string[]) {
		let computedPatchIndex = 0;

		commits.forEach((patchId: string) => {
			computedPatchIndex += 1;
			this.dropzoneIndexes.set(patchId, computedPatchIndex);
		});

		console.log('indexer.dropzoneIndexes', this.dropzoneIndexes);
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
	distanceToOtherCommit(key: string) {
		const commitIndex = this.commitIndex(key);
		if (commitIndex === undefined) return 0;

		const offset = this.index - commitIndex;

		if (offset < 0) {
			return offset + 1;
		} else {
			return offset;
		}
	}

	private commitIndex(key: string) {
		return this.commitIndexes.get(key);
	}
}

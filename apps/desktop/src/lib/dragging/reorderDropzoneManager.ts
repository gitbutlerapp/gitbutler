import { DraggableCommit } from '$lib/dragging/draggables';
import type { BranchController } from '$lib/vbranches/branchController';
// import type { VirtualBranch, DetailedCommit, PatchSeries } from '$lib/vbranches/types';

// Exported for type access only
export class ReorderDropzone {
	constructor(
		private branchController: BranchController,
		private seriesName: string,
		private entry: Entry
	) {}

	accepts(data: any) {
		console.log('accepts.data', data);
		if (!(data instanceof DraggableCommit)) return false;
		if (data.branchId !== this.seriesName) return false;
		if (this.entry.distanceToOtherCommit(data.commit.id) === 0) return false;

		return true;
	}

	onDrop(data: any) {
		console.log('drop.data', data);
		if (!(data instanceof DraggableCommit)) return;
		if (data.branchId !== this.seriesName) return;

		const offset = this.entry.distanceToOtherCommit(data.commit.id);

		console.log('REORDERING.COMMIT', {
			seriesName: this.seriesName,
			commitId: data.commit.id,
			offset
		});

		// TODO: Can we get a branchId (seriesId) onto the PatchSeries?
		// The branchId is always the same for a whole lane/stack, so when dropping,
		// even if you have hte index correclty, you can't know really which series
		// your supposed to be landing in
		this.branchController.reorderCommit(this.seriesName, data.commit.id, offset);
	}
}

export class ReorderDropzoneManager {
	private indexer: Indexer;
	private branchController: BranchController;
	// private branchIds: string[];

	constructor({
		branchController,
		commits
	}: {
		branchController: BranchController;
		commits: string[];
	}) {
		this.branchController = branchController;
		// this.branchIds = branchIds;

		this.indexer = new Indexer(commits);
	}

	topDropzone(key: string) {
		const entry = this.indexer.get(key);
		// console.log('topDropzone', { key, entry });

		const [_commitId, seriesName] = key.split('|');
		return new ReorderDropzone(this.branchController, seriesName!, entry);
	}

	dropzoneBelowCommit(key: string) {
		const entry = this.indexer.get(key);
		// console.log('topDropzone', { commitId, entry });
		//
		const [_commitId, seriesName] = key.split('|');

		return new ReorderDropzone(this.branchController, seriesName!, entry);
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

		commits.map((patchId: string) => {
			computedPatchIndex += 1;
			this.dropzoneIndexes.set(patchId, computedPatchIndex);
		});

		console.log('Indexer.dropzoneIndexes', this.dropzoneIndexes);
	}

	get(key: string) {
		const index = this.getIndex(key);

		return new Entry(this.dropzoneIndexes, index ?? 0);
	}

	private getIndex(key: string) {
		console.log('getIndex.key', key, this.dropzoneIndexes.get(key));
		if (key.includes('above')) {
			return this.dropzoneIndexes.get(key); // ?? 0;
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

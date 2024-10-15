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
		commits,
		series
	}: {
		branchController: BranchController;
		branch: VirtualBranch;
		commits?: DetailedCommit[];
		series?: PatchSeries[];
	}) {
		this.branchController = branchController;
		this.branch = branch;

		console.log('constructor.commits', commits);
		// let indexerInstance: Indexer;
		// if (commits) {
		// 	indexerInstance = new Indexer(commits);
		// } else if (series) {
		// 	const commitsArray = series.flatMap((s) => s.patches);
		// 	console.log('commitsArray', commitsArray);
		// 	indexerInstance = new Indexer(commitsArray);
		// }
		// this.indexer = indexerInstance;

		this.indexer = new Indexer(commits ? commits : series);
	}

	topDropzone(seriesReference: number) {
		// let entry: Entry;
		// if (seriesName) {
		// 	entry = this.indexer.get(`${seriesName}-top`);
		// } else {
		// 	entry = this.indexer.get('top');
		// }
		const entry = this.indexer.get(`top-${seriesReference}`);

		return new ReorderDropzone(this.branchController, this.branch, entry);
	}

	dropzoneBelowCommit(commitId: string) {
		const entry = this.indexer.get(commitId);

		return new ReorderDropzone(this.branchController, this.branch, entry);
	}
}

export class ReorderDropzoneManagerFactory {
	constructor(private branchController: BranchController) {}

	build({
		branch,
		commits,
		series
	}: {
		branch: VirtualBranch;
		commits?: DetailedCommit[];
		series?: PatchSeries[];
	}) {
		return new ReorderDropzoneManager({
			branchController: this.branchController,
			branch,
			commits,
			series
		});
	}
}

// Private classes used to calculate distances between commits
class Indexer {
	private dropzoneIndexes = new Map<string, number>();
	private commitIndexes = new Map<string, number>();

	constructor(commitGroups: DetailedCommit[] | PatchSeries[]) {
		// dropzoneIndexes.set('top', 0);
		// commits.forEach((commit, index) => {
		// 	this.dropzoneIndexes.set(commit.id, index + 1);
		// 	this.commitIndexes.set(commit.id, index);
		// });
		//
		let computedPatchIndex = 0;
		commitGroups.map((series, seriesIndex) => {
			console.log('PATCHES', series);
			console.log('topIndex', this.dropzoneIndexes.size === 0 ? 0 : this.dropzoneIndexes.size); // + 1);
			this.dropzoneIndexes.set(
				`top-${series.name}`,
				this.dropzoneIndexes.size === 0 ? 0 : this.dropzoneIndexes.size // + 1
			);

			series.patches.map((patch: DetailedCommit) => {
				computedPatchIndex += 1;
				this.dropzoneIndexes.set(patch.id, computedPatchIndex + seriesIndex); // + 1); // (seriesIndex + 1));
				this.commitIndexes.set(patch.id, computedPatchIndex);
			});
		});
	}

	get(key: string) {
		const index = this.getIndex(key);

		return new Entry(this.commitIndexes, index ?? 0);
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

		// console.log('commitIndex', commitIndex);
		const offset = this.index - commitIndex;
		console.log('distanceToOtherCommit', { offset, targetIndex: this.index, myIndex: commitIndex });
		// console.log('distanceToOtherCommit.offset', {
		// 	thisIndex: this.index,
		// 	commitId,
		// 	commitIndex,
		// 	offset
		// });

		return offset;
		// if (offset > 0) {
		// 	return offset - 1;
		// } else {
		// 	return offset;
		// }
	}

	private commitIndex(commitId: string) {
		const index = this.commitIndexes.get(commitId);
		// console.log('this.commitIndex.myIndex', commitId);
		// console.log('this.commitIndex.indexes', this.commitIndexes);
		// console.log('this.commitIndex.myIndex', index);

		// TODO: Handle updated commitIds after rebasing in `commitIndexes`
		// Reordering works, but it throws errors for old commitIds that it can't find
		// anymore after rebasing, for example.
		// if (index === undefined) {
		// 	throw new Error(`Commit ${commitId} not found in commitIndexes`);
		// }

		return index;
	}
}

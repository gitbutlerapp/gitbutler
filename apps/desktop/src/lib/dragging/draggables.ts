import { key, readKey, type SelectionId } from '$lib/selection/key';
import { get, type Readable } from 'svelte/store';
import type { AnyCommit } from '$lib/commits/commit';
import type { CommitDropData } from '$lib/commits/dropHandler';
import type { AnyFile } from '$lib/files/file';
import type { TreeChange } from '$lib/hunks/change';
import type { Hunk, HunkLock } from '$lib/hunks/hunk';
import type { IdSelection } from '$lib/selection/idSelection.svelte';

export const NON_DRAGGABLE = {
	disabled: true
};

export class HunkDropData {
	constructor(
		public readonly branchId: string,
		public readonly hunk: Hunk,
		public readonly lockedTo: HunkLock[],
		public readonly commitId: string | undefined
	) {}

	get isCommitted(): boolean {
		return !!this.commitId;
	}
}

export class ChangeDropData {
	constructor(
		readonly change: TreeChange,
		/**
		 * When a a file is dragged we compare it to what is already selected,
		 * if dragged item is part of the selection we consider that to be to
		 * be dragging all of them. If it is not part of the selection, we
		 * want to ignore what is selected and only drag the actual file being
		 * dragged.
		 */
		private selection: IdSelection,
		readonly selectionId: SelectionId
	) {}

	changedPaths(params: SelectionId): string[] {
		if (this.selection.has(this.change.path, this.selectionId)) {
			return this.selection.keys(params);
		} else {
			return [key({ ...this.selectionId, path: this.change.path })];
		}
	}

	get filePaths(): string[] {
		if (this.selection.has(this.change.path, this.selectionId)) {
			const selectionKeys = this.selection.keys(this.selectionId);
			return selectionKeys.map((key) => readKey(key).path);
		} else {
			return [this.change.path];
		}
	}

	get isCommitted(): boolean {
		return this.selectionId.type === 'commit' || this.selectionId.type === 'branch';
	}
}

export class FileDropData {
	constructor(
		readonly stackId: string,
		readonly file: AnyFile,
		readonly commit: AnyCommit | undefined,
		/**
		 * When a a file is dragged we compare it to what is already selected,
		 * if dragged item is part of the selection we consider that to be to
		 * be dragging all of them. If it is not part of the selection, we
		 * want to ignore what is selected and only drag the actual file being
		 * dragged.
		 */
		private selection: Readable<AnyFile[]> | undefined
	) {}

	get files(): AnyFile[] {
		const selectedFiles = this.selection ? get(this.selection) : undefined;
		if (selectedFiles?.some((selectedFile) => selectedFile.id === this.file.id)) {
			return selectedFiles;
		} else {
			return [this.file];
		}
	}

	get isCommitted(): boolean {
		return !!this.commit;
	}
}

export type DropData = FileDropData | HunkDropData | CommitDropData | ChangeDropData;

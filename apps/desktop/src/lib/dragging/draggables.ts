import { key } from '$lib/selection/key';
import { get, type Readable } from 'svelte/store';
import type { AnyCommit, DetailedCommit } from '$lib/commits/commit';
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
		readonly stackId: string,
		readonly file: TreeChange,
		/**
		 * When a a file is dragged we compare it to what is already selected,
		 * if dragged item is part of the selection we consider that to be to
		 * be dragging all of them. If it is not part of the selection, we
		 * want to ignore what is selected and only drag the actual file being
		 * dragged.
		 */
		private selection: IdSelection,
		readonly commitId?: string
	) {}

	changedPaths(): string[] {
		if (this.selection.has(this.file.path, this.commitId)) {
			return this.selection.values().map((value) => `${value.path}:${value.commitId}`);
		} else {
			return [key(this.file.path, this.commitId)];
		}
	}

	get isCommitted(): boolean {
		return !!this.commitId;
	}
}

export class FileDropData {
	constructor(
		readonly branchId: string,
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

export class CommitDropData {
	constructor(
		public readonly branchId: string,
		public readonly commit: DetailedCommit,
		public readonly isHeadCommit: boolean,
		public readonly seriesName?: string
	) {}
}

export type DropData = FileDropData | HunkDropData | CommitDropData | ChangeDropData;

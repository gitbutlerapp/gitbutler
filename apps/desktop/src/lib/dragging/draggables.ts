import type {
	AnyCommit,
	AnyFile,
	DetailedCommit,
	Hunk,
	Commit,
	HunkLock
} from '../vbranches/types';

export function nonDraggable() {
	return {
		disabled: true,
		data: undefined
	};
}

export class DraggableHunk {
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

export class DraggableFile {
	constructor(
		public readonly branchId: string,
		public file: AnyFile,
		public commit: AnyCommit | undefined,
		private selection: AnyFile[] | undefined
	) {}

	get files(): AnyFile[] {
		if (this.selection && this.selection.length > 0) return this.selection;
		return [this.file];
	}

	get isCommitted(): boolean {
		return !!this.commit;
	}
}

export class DraggableCommit {
	constructor(
		public readonly branchId: string,
		public readonly commit: DetailedCommit,
		public readonly isHeadCommit: boolean,
		public readonly seriesName?: string
	) {}
}

export class DraggableRemoteCommit {
	constructor(
		public readonly branchId: string,
		public readonly remoteCommit: Commit
	) {}
}

export type Draggable = DraggableFile | DraggableHunk | DraggableCommit;

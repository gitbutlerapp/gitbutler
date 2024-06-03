import type { AnyCommit, AnyFile, Commit, Hunk, RemoteCommit } from '../vbranches/types';

export function nonDraggable() {
	return {
		disabled: true,
		data: undefined
	};
}

export class DraggableHunk {
	constructor(
		public readonly branchId: string,
		public readonly hunk: Hunk
	) {}
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
}

export class DraggableCommit {
	constructor(
		public readonly branchId: string,
		public readonly commit: Commit,
		public readonly isHeadCommit: boolean
	) {}
}

export class DraggableRemoteCommit {
	constructor(
		public readonly branchId: string,
		public readonly remoteCommit: RemoteCommit
	) {}
}

export type Draggable = DraggableFile | DraggableHunk | DraggableCommit;

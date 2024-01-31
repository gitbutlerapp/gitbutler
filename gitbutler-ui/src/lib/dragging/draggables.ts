import type { Commit, File, Hunk, RemoteCommit } from '../vbranches/types';
import type { Writable } from 'svelte/store';

export function nonDraggable() {
	return {
		disabled: true,
		data: {}
	};
}

export type DraggableHunk = {
	branchId: string;
	hunk: Hunk;
};

export function draggableHunk(branchId: string | undefined, hunk: Hunk) {
	return { data: { branchId, hunk } };
}

export function isDraggableHunk(obj: any): obj is DraggableHunk {
	return obj && obj.branchId && obj.hunk;
}

export type DraggableFile = {
	branchId: string;
	files: Writable<File[]>;
	current: File;
};

export function draggableFile(branchId: string, current: File, files: Writable<File[]>) {
	return { data: { branchId, current, files } };
}

export function isDraggableFile(obj: any): obj is DraggableFile {
	return obj && obj.branchId && obj.files && obj.current;
}

export type DraggableCommit = {
	branchId: string;
	commit: Commit;
};

export function draggableCommit(branchId: string, commit: Commit) {
	return { data: { branchId, commit } };
}

export function isDraggableCommit(obj: any): obj is DraggableCommit {
	return obj && obj.branchId && obj.commit;
}

export type DraggableRemoteCommit = {
	branchId: string;
	remoteCommit: RemoteCommit;
};

export function draggableRemoteCommit(branchId: string, remoteCommit: RemoteCommit) {
	return { data: { branchId, remoteCommit } };
}

export function isDraggableRemoteCommit(obj: any): obj is DraggableRemoteCommit {
	return obj && obj.branchId && obj.remoteCommit;
}

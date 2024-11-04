import { RemoteFile, type AnyFile, type LocalFile } from '$lib/vbranches/types';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { get, writable, type Readable, type Unsubscriber } from 'svelte/store';

export interface FileKey {
	fileId: string;
	commitId?: string;
}

export type SelectedFile = {
	commitId?: string;
	file: AnyFile;
};

export function stringifyKey(fileId: string, commitId?: string) {
	return fileId + '|' + commitId;
}

export function unstringifyFileKey(fileKeyString: string): string {
	return fileKeyString.split('|')[0] ?? fileKeyString;
}

export function parseFileKey(fileKeyString: string): FileKey {
	const [fileId = '', commitId] = fileKeyString.split('|');

	return {
		fileId,
		commitId: commitId === 'undefined' ? undefined : commitId
	};
}

type CallBack = (value: string[]) => void;

/**
 * Custom store for managing the set of selected files.
 */
export class FileIdSelection implements Readable<string[]> {
	// It should not be possible to select files from different
	// so we keep track of the current commit id.
	private currentCommitId: string | undefined;

	// A string based selection so we do not emit selection changes
	// when e.g. list_virtual_branches emits.
	private selection: string[];

	// Subscribed callback functions for this custom store.
	private callbacks: CallBack[];

	// If there is a commit id, this should hold the file.
	private remoteFiles = new Map<string, RemoteFile>();

	// Selected file, if selection contains only one file.
	readonly selectedFile = writable<SelectedFile | undefined>();

	// Currently selected files, refreshed when currently selected
	// id's have changed.
	readonly files = writable<AnyFile[]>([]);

	// Unsubscribe function for the readable of uncommitted files.
	private unsubscribeLocalFiles: Unsubscriber | undefined;

	constructor(
		private uncommittedFiles: Readable<LocalFile[]>,
		value: FileKey[] = []
	) {
		this.callbacks = [];
		this.selection = value.map((key) => stringifyKey(key.fileId, key.commitId));
	}

	subscribe(callback: (value: string[]) => void) {
		callback(this.selection);
		this.callbacks.push(callback);
		if (this.callbacks.length === 1) {
			this.setup();
		}
		return () => this.unsubscribe(callback);
	}

	private unsubscribe(callback: CallBack) {
		this.callbacks = this.callbacks.filter((cb) => cb !== callback);
		if (this.callbacks.length === 0) {
			this.teardown();
		}
	}

	/**
	 * Calls each subscriber with the latest selection.
	 */
	private emit() {
		for (const callback of this.callbacks) {
			callback(this.selection);
		}
	}

	/**
	 * Called when subscriber count goes from 1 -> 0.
	 */
	async setup() {
		this.unsubscribeLocalFiles = this.uncommittedFiles.subscribe(() => {
			this.clear();
		});
	}

	/**
	 * Called when subscriber count goes from 0 -> 1.
	 */
	teardown() {
		this.unsubscribeLocalFiles?.();
		this.clear();
	}

	/**
	 * Selection is managed as string values to deduplicate events, we therefore
	 * need a way of keeping a corresponding list of files up-to-date.
	 */
	async reloadFiles() {
		const files = this.selection
			.map((selected) => {
				const key = parseFileKey(selected);
				return this.findFileByKey(key);
			})
			.filter(isDefined);

		this.files.set(files);
		if (files.length === 1) {
			this.selectedFile.set({
				commitId: this.currentCommitId,
				file: files[0] as AnyFile
			});
		} else {
			this.selectedFile.set(undefined);
		}
	}

	add(file: AnyFile, commitId?: string) {
		this.selectMany([file], commitId);
	}

	selectMany(files: AnyFile[], commitId?: string) {
		if (this.selection.length > 0 && commitId !== this.currentCommitId) {
			throw 'Selecting files from multiple commits not allowed.';
		}
		for (const file of files) {
			const fileKey = stringifyKey(file.id, commitId);
			if (!this.selection.includes(fileKey)) {
				this.selection.push(fileKey);
				if (file instanceof RemoteFile) {
					this.remoteFiles.set(fileKey, file);
				}
			}
		}
		this.emit();
		this.reloadFiles();
	}

	has(fileId: string, commitId?: string) {
		return this.selection.includes(stringifyKey(fileId, commitId));
	}

	remove(fileId: string, commitId?: string) {
		const strFileKey = stringifyKey(fileId, commitId);
		this.selection = this.selection.filter((key) => key !== strFileKey);
		if (commitId) {
			this.remoteFiles.delete(strFileKey);
		}
		if (this.selection.length === 0) {
			this.clear();
		} else {
			this.reloadFiles();
			this.emit();
		}
	}

	clear() {
		this.selection = [];
		this.remoteFiles.clear();
		this.currentCommitId = undefined;
		this.selectedFile.set(undefined);
		this.emit();
	}

	clearExcept(fileId: string, commitId?: string) {
		this.selection = [stringifyKey(fileId, commitId)];
		this.reloadFiles();
		this.emit();
	}

	only(): FileKey | undefined {
		if (this.selection.length === 0) return;
		const fileKey = parseFileKey(this.selection[0]!);
		return fileKey;
	}

	findFileByKey(key: FileKey) {
		if (key.commitId) {
			return this.remoteFiles.get(stringifyKey(key.fileId, key.commitId));
		} else {
			return get(this.uncommittedFiles).find((file) => file.id === key.fileId);
		}
	}
}

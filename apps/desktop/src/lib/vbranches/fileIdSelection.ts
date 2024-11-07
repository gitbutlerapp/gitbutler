import { RemoteFile, type AnyFile, type LocalFile } from '$lib/vbranches/types';
import { isDefined } from '@gitbutler/ui/utils/typeguards';
import { writable, type Readable } from 'svelte/store';

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
	private selection: string[] = [];

	// Subscribed callback functions for this custom store.
	private callbacks: CallBack[] = [];

	// If there is a commit id, this should hold the file.
	private remoteFiles = new Map<string, RemoteFile>();

	// Selected file, if selection contains only one file.
	readonly selectedFile = writable<SelectedFile | undefined>();

	// Currently selected files, refreshed when currently selected
	// id's have changed.
	readonly files = writable<AnyFile[]>([]);

	// For ucommitted changes we already have the files at hand,
	// while for committed changes we load the files on-demand.
	private uncommittedFiles: LocalFile[] = [];

	constructor() {}

	subscribe(callback: (value: string[]) => void) {
		callback(this.selection);
		this.callbacks.push(callback);
		return () => this.unsubscribe(callback);
	}

	private unsubscribe(callback: CallBack) {
		this.callbacks = this.callbacks.filter((cb) => cb !== callback);
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
	async setUncommittedFiles(files: LocalFile[]) {
		this.uncommittedFiles = files;
		if (this.currentCommitId !== undefined) {
			// Selections from a commit are unaffected by uncommitted files
			// so we return early.
			return;
		}
		// Remove any selections that are no longer present in the workspace.
		const localFilenames = files.map((f) => f.path);
		const removedFiles = this.selection.filter(
			(s) => !localFilenames.includes(unstringifyFileKey(s))
		);
		if (removedFiles.length > 0) {
			this.removeMany(removedFiles);
		}
		this.reloadFiles();
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

	async add(file: AnyFile, commitId?: string) {
		this.selectMany([file], commitId);
	}

	async selectMany(files: AnyFile[], commitId?: string) {
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
		this.currentCommitId = commitId;
		await this.reloadFiles();
		this.emit();
	}

	async set(file: AnyFile, commitId?: string) {
		this.clearInternal();
		this.add(file, commitId);
	}

	has(fileId: string, commitId?: string) {
		return this.selection.includes(stringifyKey(fileId, commitId));
	}

	async remove(fileId: string, commitId?: string) {
		await this.removeMany([stringifyKey(fileId, commitId)]);
	}

	async removeMany(keys: string[]) {
		this.selection = this.selection.filter((key) => !keys.includes(key));
		for (const key of keys) {
			const parsedKey = parseFileKey(key);
			if (parsedKey.commitId) {
				this.remoteFiles.delete(stringifyKey(parsedKey.fileId, parsedKey.commitId));
			}
		}
		if (this.selection.length === 0) {
			this.clear();
		} else {
			await this.reloadFiles();
			this.emit();
		}
	}

	clear() {
		this.clearInternal();
		this.emit();
	}

	// Used internally for to bypass emitting new values.
	private clearInternal() {
		this.files.set([]);
		this.selection = [];
		this.remoteFiles.clear();
		this.currentCommitId = undefined;
		this.selectedFile.set(undefined);
	}

	async clearExcept(fileId: string, commitId?: string) {
		this.selection = [stringifyKey(fileId, commitId)];
		await this.reloadFiles();
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
			return this.uncommittedFiles.find((file) => file.id === key.fileId);
		}
	}
}

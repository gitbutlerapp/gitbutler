import { writable } from 'svelte/store';

export type SelectedFile = {
	context?: string;
	fileId: string;
};
export class FileSelection {
	private _fileIds = new Set<string>();
	readonly fileIds = writable<string[]>([]);

	constructor() {}

	add(fileId: string, context?: string) {
		this._fileIds.add(context ? fileId + '|' + context : fileId);
		this.fileIds.set([...this._fileIds.values()]);
	}

	has(fileId: string, context?: string) {
		return this._fileIds.has(context ? fileId + '|' + context : fileId);
	}

	remove(fileId: string, context?: string) {
		this._fileIds.delete(context ? fileId + '|' + context : fileId);
		this.fileIds.set([...this._fileIds.values()]);
	}

	map<T>(callback: (fileId: string) => T) {
		return [...this._fileIds.values()].map((fileId) => callback(fileId));
	}

	clear() {
		this._fileIds.clear();
		this.fileIds.set([]);
	}

	get length() {
		return this._fileIds.size;
	}

	toOnly() {
		const [fileId, context] = [...this._fileIds.values()][0].split('|');
		return { fileId, context };
	}
}

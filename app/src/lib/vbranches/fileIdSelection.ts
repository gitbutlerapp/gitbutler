export function fileKey(fileId: string, commitId?: string) {
	return fileId + '|' + commitId;
}

export type SelectedFile = {
	context?: string;
	fileId: string;
};

type CallBack = (value: string[]) => void;

export class FileIdSelection {
	private value: string[];
	private callbacks: CallBack[];

	constructor() {
		this.callbacks = [];
		this.value = [];
	}

	subscribe(callback: (value: string[]) => void) {
		callback(this.value);
		this.callbacks.push(callback);
		return () => this.unsubscribe(callback);
	}

	unsubscribe(callback: CallBack) {
		this.callbacks = this.callbacks.filter((cb) => cb !== callback);
	}

	add(fileId: string, commitId?: string) {
		this.value.push(fileKey(fileId, commitId));
		this.emit();
	}

	has(fileId: string, commitId?: string) {
		return this.value.includes(fileKey(fileId, commitId));
	}

	remove(fileId: string, commitId?: string) {
		this.value = this.value.filter((key) => key != fileKey(fileId, commitId));
		this.emit();
	}

	map<T>(callback: (fileId: string) => T) {
		return this.value.map((fileKey) => callback(fileKey));
	}

	set(values: string[]) {
		this.value = values;
		this.emit();
	}

	clear() {
		this.value = [];
		this.emit();
	}

	emit() {
		for (const callback of this.callbacks) {
			callback(this.value);
		}
	}

	only() {
		const [fileId, commitId] = this.value[0].split('|');
		return { fileId, commitId };
	}

	get length() {
		return this.value.length;
	}
}

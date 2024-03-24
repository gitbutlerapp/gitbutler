import type { Branch, AnyFile } from './types';

export function filesToOwnership(files: AnyFile[]) {
	return files
		.map((f) => `${f.path}:${f.hunks.map(({ id, hash }) => `${id}-${hash}`).join(',')}`)
		.join('\n');
}

export class Ownership {
	files: Map<string, Set<string>>;

	static default() {
		return new Ownership(new Map());
	}

	static fromBranch(branch: Branch) {
		const files = branch.files.reduce((acc, file) => {
			if (acc.has(file.id)) {
				const existing = acc.get(file.id);
				file.hunks.forEach((hunk) => existing.add(hunk.id));
			} else {
				acc.set(
					file.id,
					file.hunks.reduce((acc, hunk) => {
						acc.add(hunk.id);
						return acc;
					}, new Set())
				);
			}
			return acc;
		}, new Map());
		return new Ownership(files);
	}

	constructor(files: Map<string, Set<string>>) {
		this.files = files;
	}

	removeHunk(fileId: string, ...hunkIds: string[]) {
		const hunks = this.files.get(fileId);
		if (hunks) {
			hunkIds.forEach((hunkId) => hunks.delete(hunkId));
			if (hunks.size === 0) this.files.delete(fileId);
		}
		return this;
	}

	addHunk(fileId: string, ...hunkIds: string[]) {
		const hunks = this.files.get(fileId);
		if (hunks) {
			hunkIds.forEach((hunkId) => hunks.add(hunkId));
		} else {
			this.files.set(fileId, new Set(hunkIds));
		}
		return this;
	}

	containsHunk(fileId: string, ...hunkIds: string[]): boolean {
		return hunkIds.every((hunkId) => !!this.files.get(fileId)?.has(hunkId));
	}

	clear() {
		this.files.clear();
		return this;
	}

	toString() {
		return Array.from(this.files.entries())
			.map(([fileId, hunks]) => fileId + ':' + Array.from(hunks.values()).join(','))
			.join('\n');
	}

	isEmpty() {
		return this.files.size == 0;
	}
}

import type { Branch } from './types';

export class Ownership {
	files: Map<string, Set<string>>;

	constructor(branch: Branch) {
		this.files = branch.files.reduce((acc, file) => {
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
	}

	removeHunk(fileId: string, hunkId: string) {
		const hunks = this.files.get(fileId);
		if (hunks) {
			hunks.delete(hunkId);
			if (hunks.size === 0) this.files.delete(fileId);
		}
		console.log('after remove', this.toString());
		return this;
	}

	addHunk(fileId: string, hunkId: string) {
		const hunks = this.files.get(fileId);
		if (hunks) {
			hunks.add(hunkId);
		} else {
			this.files.set(fileId, new Set([hunkId]));
		}
		return this;
	}

	containsHunk(fileId: string, hunkId: string): boolean {
		return !!this.files.get(fileId)?.has(hunkId);
	}

	toString() {
		return Array.from(this.files.entries())
			.map(([fileId, hunks]) => fileId + ':' + Array.from(hunks.values()).join(','))
			.join('\n');
	}
}

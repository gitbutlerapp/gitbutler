import type { Branch, AnyFile, Hunk, RemoteHunk, RemoteFile } from './types';

export function filesToOwnership(files: AnyFile[]) {
	return files
		.map((f) => `${f.path}:${f.hunks.map(({ id, hash }) => `${id}-${hash}`).join(',')}`)
		.join('\n');
}

export function filesToSimpleOwnership(files: RemoteFile[]) {
	return files
		.map(
			(f) =>
				`${f.path}:${f.hunks.map(({ new_start, new_lines }) => `${new_start}-${new_start + new_lines}`).join(',')}`
		)
		.join('\n');
}

// These types help keep track of what maps to what.
// TODO: refactor code for clarity, these types should not be needed
export type AnyHunk = Hunk | RemoteHunk;
export type HunkId = string;
export type FilePath = string;
export type HunkClaims = Map<HunkId, AnyHunk>;
export type FileClaims = Map<FilePath, HunkClaims>;

export class Ownership {
	private claims: FileClaims;

	static fromBranch(branch: Branch) {
		const files = branch.files.reduce((acc, file) => {
			const existing = acc.get(file.id);
			if (existing) {
				file.hunks.forEach((hunk) => existing.set(hunk.id, hunk));
			} else {
				acc.set(
					file.id,
					file.hunks.reduce((acc2, hunk) => {
						return acc2.set(hunk.id, hunk);
					}, new Map<string, AnyHunk>())
				);
			}
			return acc;
		}, new Map<FilePath, Map<HunkId, AnyHunk>>());
		const ownership = new Ownership(files);
		return ownership;
	}

	constructor(files: FileClaims) {
		this.claims = files;
	}

	remove(fileId: string, ...hunkIds: string[]) {
		const claims = this.claims;
		if (!claims) return this;
		hunkIds.forEach((hunkId) => {
			claims.get(fileId)?.delete(hunkId);
			if (claims.get(fileId)?.size === 0) claims.delete(fileId);
		});
		return this;
	}

	add(fileId: string, ...items: AnyHunk[]) {
		const claim = this.claims.get(fileId);
		if (claim) {
			items.forEach((hunk) => claim.set(hunk.id, hunk));
		} else {
			this.claims.set(
				fileId,
				items.reduce((acc, hunk) => {
					return acc.set(hunk.id, hunk);
				}, new Map<string, AnyHunk>())
			);
		}
		return this;
	}

	contains(fileId: string, ...hunkIds: string[]): boolean {
		return hunkIds.every((hunkId) => !!this.claims.get(fileId)?.has(hunkId));
	}

	clear() {
		this.claims.clear();
		return this;
	}

	toString() {
		return Array.from(this.claims.entries())
			.map(
				([fileId, hunkMap]) =>
					fileId +
					':' +
					Array.from(hunkMap.values())
						.map((hunk) => {
							return `${hunk.id}-${hunk.hash}`;
						})
						.join(',')
			)
			.join('\n');
	}

	isEmpty() {
		return this.claims.size === 0;
	}
}

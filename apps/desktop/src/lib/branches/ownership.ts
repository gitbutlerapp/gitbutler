import { InjectionToken } from '@gitbutler/shared/context';
import type { BranchStack } from '$lib/branches/branch';
import type { AnyFile } from '$lib/files/file';
import type { RemoteHunk } from '$lib/hunks/hunk';
import type { Hunk } from '$lib/hunks/hunk';
import type { Writable } from 'svelte/store';

// These types help keep track of what maps to what.
// TODO: refactor code for clarity, these types should not be needed
export type AnyHunk = Hunk | RemoteHunk;
export type HunkId = string;
export type FilePath = string;
export type HunkClaims = Map<HunkId, AnyHunk>;
export type FileClaims = Map<FilePath, HunkClaims>;

function branchFilesToClaims(files: AnyFile[]): FileClaims {
	const selection = new Map<FilePath, HunkClaims>();
	for (const file of files) {
		const existingFile = selection.get(file.id);
		if (existingFile) {
			file.hunks.forEach((hunk) => existingFile.set(hunk.id, hunk));
			continue;
		}

		selection.set(
			file.id,
			file.hunks.reduce((acc, hunk) => {
				return acc.set(hunk.id, hunk);
			}, new Map<string, AnyHunk>())
		);
	}

	return selection;
}

function selectAddedClaims(
	stack: BranchStack,
	previousState: SelectedOwnershipState,
	selection: Map<string, HunkClaims>
) {
	for (const file of stack.files) {
		const existingFile = previousState.claims.get(file.id);

		if (!existingFile) {
			// Select newly added files
			selection.set(
				file.id,
				file.hunks.reduce((acc, hunk) => {
					return acc.set(hunk.id, hunk);
				}, new Map<string, AnyHunk>())
			);
			continue;
		}

		for (const hunk of file.hunks) {
			const existingHunk = existingFile.get(hunk.id);
			if (!existingHunk) {
				// Select newly added hunks
				const existingFile = selection.get(file.id);
				if (existingFile) {
					existingFile.set(hunk.id, hunk);
				} else {
					selection.set(file.id, new Map([[hunk.id, hunk]]));
				}
			}
		}
	}
}

function ignoreRemovedClaims(
	previousState: SelectedOwnershipState,
	stack: BranchStack,
	selection: Map<string, HunkClaims>
) {
	for (const [fileId, hunkClaims] of previousState.selection.entries()) {
		const branchFile = stack.files.find((f) => f.id === fileId);
		if (branchFile) {
			for (const hunkId of hunkClaims.keys()) {
				const branchHunk = branchFile.hunks.find((h) => h.id === hunkId);
				if (branchHunk) {
					// Re-select hunks that are still present in the branch
					const existingFile = selection.get(fileId);
					if (existingFile) {
						existingFile.set(hunkId, branchHunk);
					} else {
						selection.set(fileId, new Map([[hunkId, branchHunk]]));
					}
				}
			}
		}
	}
}

interface SelectedOwnershipState {
	claims: FileClaims;
	selection: FileClaims;
}

function getState(
	stack: BranchStack,
	previousState?: SelectedOwnershipState
): SelectedOwnershipState {
	const claims = branchFilesToClaims(stack.files);

	if (previousState !== undefined) {
		const selection = new Map<FilePath, HunkClaims>();
		selectAddedClaims(stack, previousState, selection);
		ignoreRemovedClaims(previousState, stack, selection);

		return { selection, claims };
	}

	return { selection: claims, claims };
}

export const SELECTED_OWNERSHIP = new InjectionToken<Writable<SelectedOwnership>>(
	'SelectedOwnership'
);

export class SelectedOwnership {
	private claims: FileClaims;
	private selection: FileClaims;

	constructor(state: SelectedOwnershipState) {
		this.claims = state.claims;
		this.selection = state.selection;
	}

	static fromBranch(stack: BranchStack) {
		const state = getState(stack);
		const ownership = new SelectedOwnership(state);
		return ownership;
	}

	update(stack: BranchStack) {
		const { selection, claims } = getState(stack, {
			claims: this.claims,
			selection: this.selection
		});

		this.claims = claims;
		this.selection = selection;

		return this;
	}

	ignore(fileId: string, ...hunkIds: string[]) {
		const selection = this.selection;
		if (!selection) return this;
		hunkIds.forEach((hunkId) => {
			selection.get(fileId)?.delete(hunkId);
			if (selection.get(fileId)?.size === 0) selection.delete(fileId);
		});
		return this;
	}

	select(fileId: string, ...items: AnyHunk[]) {
		const selectedFile = this.selection.get(fileId);
		if (selectedFile) {
			items.forEach((hunk) => selectedFile.set(hunk.id, hunk));
		} else {
			this.selection.set(
				fileId,
				items.reduce((acc, hunk) => {
					return acc.set(hunk.id, hunk);
				}, new Map<string, AnyHunk>())
			);
		}
		return this;
	}

	isSelected(fileId: string, ...hunkIds: string[]): boolean {
		return hunkIds.every((hunkId) => !!this.selection.get(fileId)?.has(hunkId));
	}

	clearSelection() {
		this.selection.clear();
		return this;
	}

	toString() {
		return Array.from(this.selection.entries())
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

	nothingSelected() {
		return this.selection.size === 0;
	}
}

import type { BrandedId } from '@gitbutler/shared/utils/branding';

const SELECTION_TYPES = ['commit', 'branch', 'worktree'] as const;

export type SelectionType = (typeof SELECTION_TYPES)[number];

function isSelectionType(value: unknown): value is SelectionType {
	return typeof value === 'string' && SELECTION_TYPES.includes(value as SelectionType);
}

type BaseSelectionFile = {
	type: SelectionType;
	path: string;
};

type CommitSelectionParams = {
	type: 'commit';
	commitId: string;
};

type CommitSelectionFile = BaseSelectionFile & CommitSelectionParams;

type BranchSelectionParams = {
	type: 'branch';
	stackId: string;
	branchName: string;
};

type BranchSelectionFile = BaseSelectionFile & BranchSelectionParams;

type WorktreeSelectionParams = {
	type: 'worktree';
};

type WorktreeSelectionFile = BaseSelectionFile & WorktreeSelectionParams;

/**
 * Represents a selected file, can typically have a context menu
 * and/or be dragged.
 */
export type SelectedFile = CommitSelectionFile | BranchSelectionFile | WorktreeSelectionFile;

export type SelectionParameters =
	| CommitSelectionParams
	| BranchSelectionParams
	| WorktreeSelectionParams;

export type SelectedFileKey = BrandedId<'SelectedFileKey'>;

export function key(params: SelectedFile): SelectedFileKey {
	switch (params.type) {
		case 'commit':
			return `${params.type}:${params.path}:${params.commitId}` as SelectedFileKey;
		case 'branch':
			return `${params.type}:${params.path}:${params.stackId}:${params.branchName}` as SelectedFileKey;
		case 'worktree':
			return `${params.type}:${params.path}` as SelectedFileKey;
	}
}

export function readKey(key: SelectedFileKey): SelectedFile {
	const [type, ...parts] = key.split(':');

	if (!isSelectionType(type)) throw new Error('Invalid selection key');

	switch (type) {
		case 'commit':
			if (parts.length !== 2) throw new Error('Invalid commit key');
			return {
				type,
				path: parts[0]!,
				commitId: parts[1]!
			};
		case 'branch':
			if (parts.length !== 3) throw new Error('Invalid branch key');
			return {
				type,
				path: parts[0]!,
				stackId: parts[1]!,
				branchName: parts[2]!
			};
		case 'worktree':
			if (parts.length !== 1) throw new Error('Invalid worktree key');
			return {
				type,
				path: parts[0]!
			};
	}
}

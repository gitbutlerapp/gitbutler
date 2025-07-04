import type { BrandedId } from '@gitbutler/shared/utils/branding';

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = '\u001F';

const SELECTION_TYPES = ['commit', 'branch', 'worktree', 'snapshot'] as const;

export type SelectionIdType = (typeof SELECTION_TYPES)[number];

function isSelectionType(value: unknown): value is SelectionIdType {
	return typeof value === 'string' && SELECTION_TYPES.includes(value as SelectionIdType);
}

export type SelectionId = {
	type: SelectionIdType;
} & (
	| {
			type: 'worktree';
			stackId?: string;
	  }
	| {
			type: 'commit';
			commitId: string;
	  }
	| {
			type: 'branch';
			stackId?: string;
			branchName: string;
	  }
	| {
			/** Represents the selection of a change between two snapshot diffs */
			type: 'snapshot';
			/** The SHA of the before shapshot */
			before: string;
			/** The SHA of the after shapshot */
			after: string;
	  }
);

/**
 * Represents a selected file, can typically have a context menu
 * and/or be dragged.
 */
export type SelectedFile = SelectionId & { path: string };
export type SelectedFileKey = BrandedId<'SelectedFileKey'>;

export function key(params: SelectedFile): SelectedFileKey {
	switch (params.type) {
		case 'commit':
			return `${params.type}${UNIT_SEP}${params.path}${UNIT_SEP}${params.commitId}` as SelectedFileKey;
		case 'branch':
			return `${params.type}${UNIT_SEP}${params.path}${UNIT_SEP}${params.stackId}${UNIT_SEP}${params.branchName}` as SelectedFileKey;
		case 'worktree':
			return `${params.type}${UNIT_SEP}${params.path}${UNIT_SEP}${params.stackId}` as SelectedFileKey;
		case 'snapshot':
			return `${params.type}${UNIT_SEP}${params.before}${UNIT_SEP}${params.after}${UNIT_SEP}${params.path}` as SelectedFileKey;
	}
}

export function readKey(key: SelectedFileKey): SelectedFile {
	const [type, ...parts] = key.split(UNIT_SEP);

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
				// TODO: Fix this by adding a new type for regular branches.
				stackId: parts[1] === 'undefined' ? undefined : parts[1]!,
				branchName: parts[2]!
			};
		case 'worktree':
			if (parts.length !== 2) throw new Error('Invalid worktree key');
			return {
				type,
				path: parts[0]!,
				stackId: parts[1] === 'undefined' ? undefined : parts[1]
			};
		case 'snapshot':
			if (parts.length !== 3) throw new Error('Invalid snapshot key');
			return {
				type,
				before: parts[0]!,
				after: parts[1]!,
				path: parts[2]!
			};
	}
}

export function selectionKey(id: SelectionId): SelectedFileKey {
	switch (id.type) {
		case 'commit':
			return `${id.type}${UNIT_SEP}${id.commitId}` as SelectedFileKey;
		case 'branch':
			return `${id.type}${UNIT_SEP}${id.stackId}${UNIT_SEP}${id.branchName}` as SelectedFileKey;
		case 'worktree':
			return `${id.type}${UNIT_SEP}${id.stackId}` as SelectedFileKey;
		case 'snapshot':
			return `${id.type}${UNIT_SEP}${id.before}${UNIT_SEP}${id.after}` as SelectedFileKey;
	}
}

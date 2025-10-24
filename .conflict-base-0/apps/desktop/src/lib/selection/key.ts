import type { BrandedId } from '@gitbutler/shared/utils/branding';

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = '\u001F';
const UNDEFINED_REMOTE = '<<no-remote>>';

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
			stackId?: string;
	  }
	| {
			type: 'branch';
			stackId?: string;
			remote: string | undefined;
			branchName: string;
	  }
	| {
			type: 'snapshot';
			snapshotId: string;
	  }
);

/**
 * Represents a selected file, can typically have a context menu
 * and/or be dragged.
 */
export type SelectedFile = SelectionId & { path: string };
export type SelectedFileKey = BrandedId<'SelectedFileKey'>;
export type SelectionKey = BrandedId<'SelectedKey'>;
export type StableSelectionKey = BrandedId<'StableSelectedKey'>;

export function key(params: SelectedFile): SelectedFileKey {
	switch (params.type) {
		case 'commit':
			return `${params.type}${UNIT_SEP}${params.path}${UNIT_SEP}${params.commitId}${UNIT_SEP}${params.stackId}` as SelectedFileKey;
		case 'branch':
			return `${params.type}${UNIT_SEP}${params.path}${UNIT_SEP}${params.stackId}${UNIT_SEP}${params.remote ?? UNDEFINED_REMOTE}${UNIT_SEP}${params.branchName}` as SelectedFileKey;
		case 'worktree':
			return `${params.type}${UNIT_SEP}${params.path}${UNIT_SEP}${params.stackId}` as SelectedFileKey;
		case 'snapshot':
			return `${params.type}${UNIT_SEP}${params.snapshotId}${UNIT_SEP}${params.path}` as SelectedFileKey;
	}
}

export function readKey(key: SelectedFileKey): SelectedFile {
	const [type, ...parts] = key.split(UNIT_SEP);

	if (!isSelectionType(type)) throw new Error('Invalid selection key');

	switch (type) {
		case 'commit':
			if (parts.length !== 3) throw new Error('Invalid commit key');
			return {
				type,
				path: parts[0]!,
				commitId: parts[1]!,
				stackId: parts[2]!
			};
		case 'branch':
			if (parts.length !== 4) throw new Error('Invalid branch key');
			return {
				type,
				path: parts[0]!,
				// TODO: Fix this by adding a new type for regular branches.
				stackId: parts[1] === 'undefined' ? undefined : parts[1]!,
				remote: parts[2] === UNDEFINED_REMOTE ? undefined : parts[2]!,
				branchName: parts[3]!
			};
		case 'worktree':
			if (parts.length !== 2) throw new Error('Invalid worktree key');
			return {
				type,
				path: parts[0]!,
				stackId: parts[1] === 'undefined' ? undefined : parts[1]
			};
		case 'snapshot':
			if (parts.length !== 2) throw new Error('Invalid snapshot key');
			return {
				type,
				snapshotId: parts[0]!,
				path: parts[1]!
			};
	}
}

export function selectionKey(id: SelectionId): SelectionKey {
	switch (id.type) {
		case 'commit':
			return `${id.type}${UNIT_SEP}${id.commitId}` as SelectionKey;
		case 'branch':
			return `${id.type}${UNIT_SEP}${id.stackId}${UNIT_SEP}${id.remote ?? UNDEFINED_REMOTE}${UNIT_SEP}${id.branchName}` as SelectionKey;
		case 'worktree':
			return `${id.type}${UNIT_SEP}${id.stackId}` as SelectionKey;
		case 'snapshot':
			return `${id.type}${UNIT_SEP}${id.snapshotId}` as SelectionKey;
	}
}

/**
 * Like selectionKey but includes the type in order to be read back unambiguously.
 */
export function stableSelectionKey(id: SelectionId): StableSelectionKey {
	switch (id.type) {
		case 'commit':
			return `commit${UNIT_SEP}${id.commitId}${UNIT_SEP}${id.stackId ?? 'undefined'}` as StableSelectionKey;
		case 'branch':
			return `branch${UNIT_SEP}${id.stackId ?? 'undefined'}${UNIT_SEP}${id.remote ?? UNDEFINED_REMOTE}${UNIT_SEP}${id.branchName}` as StableSelectionKey;
		case 'worktree':
			return `worktree${UNIT_SEP}${id.stackId ?? 'undefined'}` as StableSelectionKey;
		case 'snapshot':
			return `snapshot${UNIT_SEP}${id.snapshotId}` as StableSelectionKey;
	}
}

/**
 * Read the stable selection key back into a SelectionId.
 */
export function readStableSelectionKey(key: StableSelectionKey): SelectionId {
	const [type, ...parts] = key.split(UNIT_SEP);

	if (!isSelectionType(type)) throw new Error('Invalid selection key');

	switch (type) {
		case 'commit':
			if (parts.length !== 2) throw new Error('Invalid commit key');
			return {
				type,
				commitId: parts[0]!,
				stackId: parts[1] === 'undefined' ? undefined : parts[1]!
			};
		case 'branch':
			if (parts.length !== 3) throw new Error('Invalid branch key');
			return {
				type,
				stackId: parts[0] === 'undefined' ? undefined : parts[0]!,
				remote: parts[1] === UNDEFINED_REMOTE ? undefined : parts[1]!,
				branchName: parts[2]!
			};
		case 'worktree':
			if (parts.length !== 1) throw new Error('Invalid worktree key');
			return {
				type,
				stackId: parts[0] === 'undefined' ? undefined : parts[0]
			};
		case 'snapshot':
			if (parts.length !== 1) throw new Error('Invalid snapshot key');
			return {
				type,
				snapshotId: parts[0]!
			};
	}
}

/**
 * Helper function to create SelectionId objects with auto-populated laneId.
 * Uses stackId as laneId if it exists, otherwise defaults to "banana".
 */
export function createSelectionId<T extends Omit<SelectionId, 'laneId'>>(params: T): SelectionId {
	const stackId = 'stackId' in params ? (params as any).stackId : undefined;
	const laneId: string = stackId || 'banana';
	return { ...params, laneId } as any;
}

/**
 * Helper functions for creating specific SelectionId types with auto-populated laneId
 */
export function createWorktreeSelection(params: { stackId?: string }): SelectionId {
	return createSelectionId({ type: 'worktree', stackId: params.stackId });
}

export function createCommitSelection(params: { commitId: string; stackId?: string }): SelectionId {
	return createSelectionId({ type: 'commit', commitId: params.commitId, stackId: params.stackId });
}

export function createBranchSelection(params: {
	stackId?: string;
	branchName: string;
	remote: string | undefined;
}): SelectionId {
	return createSelectionId({
		type: 'branch',
		stackId: params.stackId,
		branchName: params.branchName,
		remote: params.remote
	});
}

export function createSnapshotSelection(params: { snapshotId: string }): SelectionId {
	return createSelectionId({ type: 'snapshot', snapshotId: params.snapshotId });
}

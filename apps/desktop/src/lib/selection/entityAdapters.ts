import { platformPathSeparator } from '$lib/backend';
import { createEntityAdapter } from '@reduxjs/toolkit';
import type { TreeChange } from '$lib/hunks/change';
import type { HunkAssignment, HunkHeader } from '$lib/hunks/hunk';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = '\u001F';

/**
 * Assignments and selections are keyed by this combination of parameters.
 *
 * TODO: Do we need to serialize the whole header, or is newStart sufficient?
 */
export function compositeKey(args: {
	stackId: string | null;
	path: string;
	hunkHeader: HunkHeader | null;
}) {
	const { stackId, path, hunkHeader } = args;
	const header = hunkHeader?.newStart || hunkHeader;
	return stackId + UNIT_SEP + path + UNIT_SEP + header;
}

/**
 * Creates a partial key for matching the beginning of keys.
 */
export function partialKey(stackId: string | null, path?: string) {
	return path ? stackId + UNIT_SEP + path + UNIT_SEP : stackId + UNIT_SEP;
}

/**
 * Get the path separator for the current platform.
 *
 * We need this to filter for assignments in a specific directory.
 */

/**
 * Creates a prefix key for matching directories.
 */
export function prefixKey(stackId: string | null, path: string) {
	const separator = platformPathSeparator();
	return stackId + UNIT_SEP + path + separator;
}

export const treeChangeAdapter = createEntityAdapter<TreeChange, string>({
	selectId: (change) => change.path
});

export const hunkAssignmentAdapter = createEntityAdapter<HunkAssignment, string>({
	selectId: (c) => compositeKey(c)
});

/**
 * There may be at most one HunkSelection for each HunkAssignment. As such, we
 * use an `assignmentId` which cooresponds to a given HunkAssignment both as a
 * foreign key, but also the primary identifier of a HunkSelection.
 */
export type HunkSelection = {
	stableId: string | null;
	assignmentId: string;
	stackId: string | null;
	path: string;
	lines: LineId[];
};

export const hunkSelectionAdapter = createEntityAdapter<HunkSelection, string>({
	selectId: (c) => c.assignmentId
});

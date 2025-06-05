import { createEntityAdapter } from '@reduxjs/toolkit';
import type { TreeChange } from '$lib/hunks/change';
import type { HunkAssignment, HunkHeader } from '$lib/hunks/hunk';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

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
	if (typeof args.hunkHeader === 'string' || args.hunkHeader === null) {
		return `${args.stackId}::${args.path}::${args.hunkHeader}`;
	}
	return `${args.stackId}::${args.path}::${args.hunkHeader?.newStart || null}`;
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
	assignmentId: string;
	stackId: string | null;
	path: string;
	changeId: string;
	lines: LineId[];
};

export const hunkSelectionAdapter = createEntityAdapter<HunkSelection, string>({
	selectId: (c) => c.assignmentId
});

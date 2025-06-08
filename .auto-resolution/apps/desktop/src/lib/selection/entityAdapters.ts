import { createEntityAdapter } from '@reduxjs/toolkit';
import type { TreeChange } from '$lib/hunks/change';
import type { HunkAssignment, HunkHeader } from '$lib/hunks/hunk';
import type { LineId } from '@gitbutler/ui/utils/diffParsing';

// ASCII Unit Separator, used to separate data units within a record or field.
const UNIT_SEP = '\u001F';

// We need this to filter for assignments in a specific directory.
const PATH_SEP = platformName === 'windows' ? '\\' : '/';

/**
 * Assignments and selections are keyed by this combination of parameters.
 *
 * TODO: Do we need to serialize the whole header, or is newStart sufficient?
 */
export function compositeKey(args: {
	stackId: string | null;
	path: string;
	hunkHeader: string | HunkHeader | null;
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

export type HunkSelection = {
	hunkSelectionId: string;
	stackId: string | null;
	path: string;
	assignmentId: string;
	changeId: string;
	lines: LineId[];
};

export const hunkSelectionAdapter = createEntityAdapter<HunkSelection, string>({
	selectId: (c) => c.hunkSelectionId
});

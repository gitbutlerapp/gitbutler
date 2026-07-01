import { encodeDiffFileLine } from "@gitbutler/ui/utils/diffParsing";
import type { DiffPatch } from "@gitbutler/shared/chat/types";
import type { DiffFileLineId, LineSelector } from "@gitbutler/ui/utils/diffParsing";

export interface DiffLineSelected extends LineSelector {
	index: number;
}

export interface DiffSelection {
	diffSha: string;
	fileName: string;
	lines: DiffLineSelected[];
}

/**
 * Create a diff line selection string out of a diff patch array.
 *
 * @note - This function assumes that the diff patch array is an ordered & continues selection of lines.
 */
export function parseDiffPatchToEncodedSelection(
	fileName: string,
	diffPatchArray: DiffPatch[],
): DiffFileLineId | undefined {
	if (diffPatchArray.length === 0) return undefined;
	return encodeDiffFileLine(fileName, diffPatchArray[0].left, diffPatchArray[0].right);
}

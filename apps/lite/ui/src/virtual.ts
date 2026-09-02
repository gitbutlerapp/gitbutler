import { defaultRangeExtractor, type Range } from "@tanstack/react-virtual";

/**
 * Get a virtualisation range extractor which includes the provided indices, if any.
 *
 * Returned extractor order is unspecified; callers whose DOM order is semantic must sort it.
 */
export const getRangeExtractorWithIndices = (
	range: Range,
	idxs: ReadonlyArray<number>,
): Array<number> => {
	// The default range is contiguous.
	const defIdxs = defaultRangeExtractor(range);

	if (idxs.length === 0) return defIdxs;

	const fstIdx = defIdxs[0];
	const lastIdx = defIdxs.at(-1);

	for (const idx of idxs) {
		// Only indices in the virtualiser's current item set can be pinned.
		if (idx < 0 || idx >= range.count) continue;

		// The virtualiser positions items independently of extractor order, so an out-of-range index
		// can be safely appended in O(1). Callers that require DOM order sort the result afterward.
		if (fstIdx === undefined || lastIdx === undefined || idx < fstIdx || idx > lastIdx)
			defIdxs.push(idx);
	}

	return defIdxs;
};

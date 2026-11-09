import { defaultRangeExtractor, type Range } from "@tanstack/react-virtual";

/**
 * Get a virtualisation range extractor which includes the provided indices, if any.
 *
 * The returned indices are ascending. The virtualiser positions items independently of
 * extractor order, but the rows it renders are DOM siblings, and in a `role="tree"` DOM
 * order is what a screen reader reads and what sequential navigation follows — a pinned
 * row appended after the range would be announced last among its siblings.
 *
 * Ordering also keeps the rows still: a pin that moves between the end of the list and
 * its place in it as the range slides past re-orders React's keyed children on every
 * render, and React answers a re-order by tearing each moved row's layout effects down
 * and setting them up again. A row that virtualises its own contents against a shared
 * scroller then re-attaches to it that often, and each re-attachment writes an offset
 * back onto it — which is what used to leave these lists unable to scroll past the
 * selected row.
 */
export const getRangeExtractorWithIndices = (
	range: Range,
	idxs: ReadonlyArray<number>,
): Array<number> => {
	// The default range is contiguous and ascending.
	const defIdxs = defaultRangeExtractor(range);

	if (idxs.length === 0) return defIdxs;

	const fstIdx = defIdxs[0];
	const lastIdx = defIdxs.at(-1);
	let appended = false;

	for (const idx of idxs) {
		// Only indices in the virtualiser's current item set can be pinned.
		if (idx < 0 || idx >= range.count) continue;

		if (fstIdx === undefined || lastIdx === undefined || idx < fstIdx || idx > lastIdx) {
			defIdxs.push(idx);
			appended = true;
		}
	}

	// Only what fell outside the default range was appended, so this is a nearly sorted array
	// of a few dozen numbers at most.
	if (appended) defIdxs.sort((a, b) => a - b);

	return defIdxs;
};

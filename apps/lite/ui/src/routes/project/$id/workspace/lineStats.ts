import type { UnifiedPatch } from "@gitbutler/but-sdk";

export type LineStats = {
	linesAdded: number;
	linesRemoved: number;
};

/**
 * One change's added/removed counts, or `null` when it has no patch to count
 * (binary, or too large).
 *
 * The backend counts changed lines; the parsed patch's `additionLines` and
 * `deletionLines` hold every line present on each side, context included, so
 * reading their lengths inflates the numbers.
 */
export function patchLineStats(patch: UnifiedPatch | null | undefined): LineStats | null {
	if (patch?.type !== "Patch") return null;
	return { linesAdded: patch.subject.linesAdded, linesRemoved: patch.subject.linesRemoved };
}

export function getLineStats(diffs: Array<UnifiedPatch | null | undefined>): LineStats {
	const stats: LineStats = { linesAdded: 0, linesRemoved: 0 };
	for (const diff of diffs) {
		const fileStats = patchLineStats(diff);
		if (!fileStats) continue;
		stats.linesAdded += fileStats.linesAdded;
		stats.linesRemoved += fileStats.linesRemoved;
	}
	return stats;
}

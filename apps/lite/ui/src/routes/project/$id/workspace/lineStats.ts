import type { UnifiedPatch } from "@gitbutler/but-sdk";

export type LineStats = {
	linesAdded: number;
	linesRemoved: number;
};

export function getLineStats(diffs: Array<UnifiedPatch | null | undefined>): LineStats {
	const stats: LineStats = { linesAdded: 0, linesRemoved: 0 };
	for (const diff of diffs) {
		if (diff?.type !== "Patch") continue;
		stats.linesAdded += diff.subject.linesAdded;
		stats.linesRemoved += diff.subject.linesRemoved;
	}
	return stats;
}

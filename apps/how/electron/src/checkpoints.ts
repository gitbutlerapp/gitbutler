import type { DiffSpec, HunkHeader, TreeChange } from "@gitbutler/but-sdk";

export function createDiffSpec(
	change: TreeChange,
	hunkHeaders: Array<HunkHeader> = [],
): DiffSpec {
	return {
		pathBytes: change.pathBytes,
		previousPathBytes:
			change.status.type === "Rename" ? change.status.subject.previousPathBytes : null,
		hunkHeaders:
			change.status.type === "Addition" || change.status.type === "Deletion" ? [] : hunkHeaders,
	};
}

export function checkpointMessage(date: Date): string {
	return `Checkpoint: ${new Intl.DateTimeFormat(undefined, {
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
	}).format(date)}`;
}

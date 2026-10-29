import type { ConflictEntryPresence, FileInfo } from "@gitbutler/but-sdk";

export function emptyConflictEntryPresence(): ConflictEntryPresence {
	return {
		ancestor: false,
		ours: false,
		theirs: false,
	};
}

export function conflictEntryHint(presence: ConflictEntryPresence): string {
	let defaultVerb = "added";

	if (presence.ancestor) {
		defaultVerb = "modified";
	}

	let oursVerb = defaultVerb;

	if (!presence.ours) {
		oursVerb = "deleted";
	}

	let theirsVerb = defaultVerb;

	if (!presence.theirs) {
		theirsVerb = "deleted";
	}

	return `You have ${theirsVerb} this file, They have ${oursVerb} this file.`;
}

function looksConflicted(file: string): boolean {
	const lines = file.split("\n");
	for (const line of lines) {
		if (line.startsWith("<<<<<<<")) {
			return true;
		}
	}
	return false;
}

export type ConflictState = "conflicted" | "resolved" | "unknown";

export function getConflictState(
	conflictEntryPresence: ConflictEntryPresence,
	file: FileInfo | undefined,
): ConflictState {
	if (!conflictEntryPresence.ours || !conflictEntryPresence.theirs) {
		return "conflicted";
	}

	// No content (the file could not be read or is not valid UTF-8), or
	// base64 content (mimeType is set): we cannot scan for conflict markers.
	if (file === undefined || file.content === null || file.mimeType !== null) {
		return "unknown";
	}

	return looksConflicted(file.content) ? "conflicted" : "resolved";
}

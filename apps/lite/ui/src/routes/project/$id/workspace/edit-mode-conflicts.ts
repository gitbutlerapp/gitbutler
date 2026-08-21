import type { ConflictEntryPresence, FileInfo } from "@gitbutler/but-sdk";

/**
 * Whether a conflicted file still looks conflicted on disk. `unknown` covers
 * files we cannot judge from their text — binary, or gone.
 */
export type ConflictState = "conflicted" | "resolved" | "unknown";

/**
 * What a conflict is between, said plainly. Both sides present is the
 * ordinary conflict; a missing side means one of them deleted the file.
 */
export const conflictHint = (presence: ConflictEntryPresence): string => {
	if (presence.ours && presence.theirs) return "conflicts";
	if (!presence.ours && presence.theirs) return "deleted by you";
	if (presence.ours && !presence.theirs) return "deleted by them";
	return "conflicts";
};

const hasConflictMarker = (content: string): boolean =>
	content.split("\n").some((line) => line.startsWith("<<<<<<<"));

/**
 * A file one side deleted has no text to settle the question, so it stays
 * conflicted until the user says otherwise. Otherwise a leftover marker is
 * the signal, the same one git writes.
 */
export const conflictStateOf = (
	presence: ConflictEntryPresence,
	file: FileInfo | undefined,
): ConflictState => {
	if (!presence.ours || !presence.theirs) return "conflicted";
	// No content, or content we would have to decode as bytes: unjudgeable.
	if (file === undefined || file.content === null || file.mimeType !== null) return "unknown";
	return hasConflictMarker(file.content) ? "conflicted" : "resolved";
};

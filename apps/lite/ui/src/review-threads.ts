/**
 * @file Placing a review's diff comments in the diff view.
 *
 * The forge numbers a thread's line in the file as it last saw it — the
 * branch head. That is the version the branch diff shows, so this is where
 * the numbers mean anything; `annotation.ts` does the same job for
 * GitButler's own diff comments.
 */

import type { FileParent } from "#ui/addresses.ts";
import type { ForgeReviewThread } from "@gitbutler/but-sdk";
import type { AnnotationSide } from "@pierre/diffs";

/** A thread with the diff position the view can hang it on. */
export type AnchoredThread = {
	thread: ForgeReviewThread;
	lineNumber: number;
	side: AnnotationSide;
};

export type ThreadsByPath = ReadonlyMap<string, Array<AnchoredThread>>;

/**
 * Group the threads the branch diff can place, by path.
 *
 * Only the branch scope: a single commit's diff numbers its own version of
 * the file, so the same line number would point somewhere else. Resolved
 * and outdated threads are left out — the first is settled, the second has
 * no line left to point at — and both remain in the pull request tab.
 */
export const threadsByPathForScope = (
	threads: Array<ForgeReviewThread>,
	fileParent: FileParent,
): ThreadsByPath => {
	const byPath = new Map<string, Array<AnchoredThread>>();
	if (fileParent._tag !== "Branch") return byPath;

	for (const thread of threads) {
		if (thread.isResolved || thread.isOutdated || thread.line === null) continue;

		const anchored = byPath.get(thread.path) ?? [];
		anchored.push({
			thread,
			lineNumber: thread.line,
			// The forge's old side is the pre-image, which the diff draws as
			// its deletions.
			side: thread.side === "old" ? "deletions" : "additions",
		});
		byPath.set(thread.path, anchored);
	}

	return byPath;
};

/**
 * The line a thread hangs on, as the forge quoted it — the last line of the
 * hunk it sent, without the `+`/`-`/space marker every hunk line carries.
 * `null` when the forge sent no hunk, which leaves nothing to check.
 */
const anchoredLineText = (thread: ForgeReviewThread): string | null => {
	const diffHunk = thread.comments[0]?.diffHunk;
	if (diffHunk == null) return null;

	// The quoted line is the last diff line: a trailing newline leaves an
	// empty string behind, and a hunk at a file's unterminated end closes
	// with a `\ No newline` marker — neither is quoted code.
	const lines = diffHunk.split("\n").filter((line) => /^[ +-]/.test(line));
	const last = lines.at(-1);
	return last === undefined ? null : last.slice(1);
};

/**
 * The slice of Pierre's hunk model needed to place a file line. Each side is
 * stored as one compact array across the whole file, and a hunk says both
 * where it starts in the file and where its slice starts in that array.
 */
type PlacedHunk = {
	additionStart: number;
	additionCount: number;
	additionLineIndex: number;
	deletionStart: number;
	deletionCount: number;
	deletionLineIndex: number;
};

type DiffLines = {
	hunks: ReadonlyArray<PlacedHunk>;
	additionLines: ReadonlyArray<string | undefined>;
	deletionLines: ReadonlyArray<string | undefined>;
};

/**
 * What the diff holds at a file line number, or `undefined` when it does not
 * carry that line at all — the file's numbering only reaches the compact
 * arrays through the hunk that contains it.
 */
const diffLineText = (
	diff: DiffLines,
	side: AnnotationSide,
	lineNumber: number,
): string | undefined => {
	for (const hunk of diff.hunks) {
		const start = side === "additions" ? hunk.additionStart : hunk.deletionStart;
		const count = side === "additions" ? hunk.additionCount : hunk.deletionCount;
		const base = side === "additions" ? hunk.additionLineIndex : hunk.deletionLineIndex;
		if (lineNumber < start || lineNumber >= start + count) continue;

		const lines = side === "additions" ? diff.additionLines : diff.deletionLines;
		// Stored with the line ending the file had; the forge quotes without it.
		return lines[base + (lineNumber - start)]?.replace(/\r?\n$/, "");
	}

	return undefined;
};

/**
 * Whether the diff on screen still says what the forge quoted.
 *
 * The branch moves under a thread whenever a commit is amended or rebased,
 * and until that reaches the forge it keeps calling the thread current — so
 * its line number can point at code that has since changed. Comparing the
 * quoted line against the one the diff actually holds is what catches that.
 *
 * A line the diff does not carry is left alone: it is outside every hunk, so
 * nothing would be drawn against it either way.
 */
export const threadStillAnchored = (
	thread: ForgeReviewThread,
	lineNumber: number,
	side: AnnotationSide,
	diff: DiffLines,
): boolean => {
	const quoted = anchoredLineText(thread);
	if (quoted === null) return true;

	const actual = diffLineText(diff, side, lineNumber);
	return actual === undefined || actual === quoted;
};

/**
 * The forge's hunk as a patch a diff parser will accept.
 *
 * A hunk arrives truncated at the line the comment sits on, but still
 * carrying the `@@` counts of the whole hunk — so the header promises more
 * lines than were sent, and a parser following it runs off the end. The
 * counts are re-tallied from what actually arrived; the start lines are
 * untouched, since the truncation only ever drops the tail.
 */
export const forgeHunkPatch = (path: string, diffHunk: string): string | null => {
	const [header = "", ...rest] = diffHunk.split("\n");
	const starts = /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/.exec(header);
	const [, oldStart, newStart] = starts ?? [];
	if (oldStart === undefined || newStart === undefined) return null;

	// A blank context line is a single space; a trailing newline leaves an
	// empty string, and a hunk at a file's unterminated end carries a
	// `\ No newline` marker — dropped, since it is not a line the quote
	// needs and not every parser expects it.
	const lines = rest.filter((line) => /^[ +-]/.test(line));
	const spans = (markers: string) =>
		lines.filter((line) => markers.includes(line.slice(0, 1))).length;

	return [
		// The same header shape `synthesizeFilePatch` builds for Pierre.
		`diff --git a/${path} b/${path}`,
		`--- a/${path}`,
		`+++ b/${path}`,
		`@@ -${oldStart},${spans(" -")} +${newStart},${spans(" +")} @@`,
		...lines,
		"",
	].join("\n");
};

/**
 * Whether the file as it stands still says what the forge quoted.
 *
 * The forge's own `isOutdated` only knows the pushed head, so a branch
 * amended or rebased since keeps its threads looking current while their
 * line numbers point at code that has moved. This is the same question
 * `threadStillAnchored` asks of the diff, put to the working file.
 */
export const threadStillAnchoredInFile = (thread: ForgeReviewThread, text: string): boolean => {
	// An old-side thread quotes the pre-image, which no working file holds —
	// there is nothing to compare, so the forge's own flag stands.
	if (thread.side === "old") return true;
	const quoted = anchoredLineText(thread);
	if (quoted === null || thread.line === null) return true;

	const actual = text.split("\n")[thread.line - 1];
	// A file that no longer reaches the line has certainly moved past it.
	return actual !== undefined && actual.replace(/\r$/, "") === quoted;
};

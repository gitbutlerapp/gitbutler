import { Commit, HunkHeader } from "@gitbutler/but-sdk";

/** @public */
export const assert = <T,>(t: T | null | undefined): T => {
	if (t == null) throw new Error("Expected value to be non-null and defined");
	return t;
};

// https://linear.app/gitbutler/issue/GB-1161/refsbranches-should-use-bytes-instead-of-strings
export const decodeRefName = (fullNameBytes: Array<number>): string =>
	new TextDecoder().decode(Uint8Array.from(fullNameBytes));
export const encodeRefName = (fullName: string): Array<number> =>
	Array.from(new TextEncoder().encode(fullName));

export const formatHunkHeader = (hunk: HunkHeader): string =>
	`-${hunk.oldStart},${hunk.oldLines} +${hunk.newStart},${hunk.newLines}`;

export const shortCommitId = (commitId: string): string => commitId.slice(0, 7);

export const commitTitle = (message: string): string => {
	const _title = message.trim().split("\n")[0];
	const title = _title === "" ? undefined : _title;
	return title ?? "(no message)";
};

export type CommitConflictDisplay = "none" | "actual" | "predicted";

export const CommitLabel = ({
	commit,
	conflictDisplay = commit.hasConflicts ? "actual" : "none",
}: {
	commit: Commit;
	conflictDisplay?: CommitConflictDisplay;
}) => (
	<>
		{commitTitle(commit.message)}
		{conflictDisplay !== "none" && (
			<span style={conflictDisplay === "predicted" ? { opacity: 0.5 } : undefined}> ⚠️</span>
		)}
	</>
);

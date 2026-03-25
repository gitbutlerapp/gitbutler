import { type UIRejectedPath } from "@gitbutler/but-sdk";
import { type ToastManagerAddOptions } from "@base-ui/react";
import { Array, pipe } from "effect";
import { FC } from "react";

export const REJECTION_REASONS = [
	"noEffectiveChanges",
	"cherryPickMergeConflict",
	"workspaceMergeConflict",
	"workspaceMergeConflictOfUnrelatedFile",
	"worktreeFileMissingForObjectConversion",
	"fileToLargeOrBinary",
	"pathNotFoundInBaseTree",
	"unsupportedDirectoryEntry",
	"unsupportedTreeEntry",
	"missingDiffSpecAssociation",
	"unknown",
] as const;

export type RejectionReason = (typeof REJECTION_REASONS)[number];

export type RejectedChange = Omit<UIRejectedPath, "reason"> & {
	reason: RejectionReason;
};

function isRejectionReason(reason: string): reason is Exclude<RejectionReason, "unknown"> {
	return (
		reason === "noEffectiveChanges" ||
		reason === "cherryPickMergeConflict" ||
		reason === "workspaceMergeConflict" ||
		reason === "workspaceMergeConflictOfUnrelatedFile" ||
		reason === "worktreeFileMissingForObjectConversion" ||
		reason === "fileToLargeOrBinary" ||
		reason === "pathNotFoundInBaseTree" ||
		reason === "unsupportedDirectoryEntry" ||
		reason === "unsupportedTreeEntry" ||
		reason === "missingDiffSpecAssociation"
	);
}

export function normalizeRejectedChanges(
	pathsToRejectedChanges: Array<UIRejectedPath>,
): Array<RejectedChange> {
	return pathsToRejectedChanges.map((change) => ({
		...change,
		reason: isRejectionReason(change.reason) ? change.reason : "unknown",
	}));
}

const listFormatter = new Intl.ListFormat(undefined, {
	style: "long",
	type: "conjunction",
});

const formatRejectedPaths = (paths: Array<string>): string => {
	if (paths.length === 0) return "";
	if (paths.length <= 3) return listFormatter.format(paths);

	return listFormatter.format([...paths.slice(0, 3), `${paths.length - 3} more`]);
};

const readableRejectionReason = (reason: RejectionReason): string => {
	switch (reason) {
		case "cherryPickMergeConflict":
			return "Cherry-pick merge conflict";
		case "noEffectiveChanges":
			return "No effective changes";
		case "workspaceMergeConflict":
			return "Workspace merge conflict";
		case "workspaceMergeConflictOfUnrelatedFile":
			return "Workspace merge conflict in another file";
		case "worktreeFileMissingForObjectConversion":
			return "Worktree file missing for object conversion";
		case "fileToLargeOrBinary":
			return "File too large or binary";
		case "pathNotFoundInBaseTree":
			return "Path not found in base tree";
		case "unsupportedDirectoryEntry":
			return "Unsupported directory entry";
		case "unsupportedTreeEntry":
			return "Unsupported tree entry";
		case "missingDiffSpecAssociation":
			return "Missing diff spec association";
		case "unknown":
			return "Unknown rejection reason";
		default:
			return reason;
	}
};

const RejectedChanges: FC<{
	rejectedChanges: Array<RejectedChange>;
}> = ({ rejectedChanges }) => {
	const pathsByReason = new Map<RejectionReason, Array<string>>();

	for (const { reason, path } of rejectedChanges) {
		const paths = pathsByReason.get(reason);
		if (paths) paths.push(path);
		else pathsByReason.set(reason, [path]);
	}

	return (
		<ul>
			{pipe(
				pathsByReason,
				Array.fromIterable,
				Array.map(([reason, paths]) => (
					<li key={reason}>
						<strong>{readableRejectionReason(reason)}:</strong> {formatRejectedPaths(paths)}
					</li>
				)),
			)}
		</ul>
	);
};

export const rejectedChangesToastOptions = ({
	newCommit,
	pathsToRejectedChanges,
}: {
	newCommit?: string | null;
	pathsToRejectedChanges: Array<RejectedChange>;
}): ToastManagerAddOptions<never> => ({
	title: newCommit != null ? "Some changes were not committed" : "Failed to create commit",
	description: <RejectedChanges rejectedChanges={pathsToRejectedChanges} />,
	priority: "high",
});

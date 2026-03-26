import { type ShortcutBinding } from "#ui/shortcuts.ts";
import { type RefInfo } from "@gitbutler/but-sdk";

export type CommitDetailsSelection = {
	stackId: string;
	segmentIndex: number;
	branchName: string | null;
	branchRef: string | null;
	commitId: string;
	path?: string;
};

export const normalizeCommitDetailsSelection = (
	details: CommitDetailsSelection | null,
	headInfo: RefInfo,
): CommitDetailsSelection | null => {
	if (!details) return null;
	const stack = headInfo.stacks.find((stack) => stack.id !== null && stack.id === details.stackId);
	if (!stack) return null;
	const segment = stack.segments[details.segmentIndex];
	if (!segment) return null;
	if (!segment.commits.some((commit) => commit.id === details.commitId)) return null;
	return details;
};

export const getCommitDetailsForCommit = ({
	details,
	stackId,
	segmentIndex,
	commitId,
}: {
	details: CommitDetailsSelection | null;
	stackId: string;
	segmentIndex: number;
	commitId: string;
}): CommitDetailsSelection | null =>
	details !== null &&
	details.stackId === stackId &&
	details.segmentIndex === segmentIndex &&
	details.commitId === commitId
		? details
		: null;

export const getAdjacentCommitDetailsPath = ({
	paths,
	currentPath,
	offset,
}: {
	paths: Array<string>;
	currentPath: string | undefined;
	offset: -1 | 1;
}): string | null => {
	if (paths.length === 0) return null;
	if (currentPath === undefined) return offset > 0 ? (paths[0] ?? null) : (paths.at(-1) ?? null);

	const currentIndex = paths.indexOf(currentPath);
	if (currentIndex === -1) return offset > 0 ? (paths[0] ?? null) : (paths.at(-1) ?? null);
	return paths[currentIndex + offset] ?? null;
};

type CommitDetailsSelectionAction = { _tag: "Move"; offset: -1 | 1 } | { _tag: "Close" };

export const commitDetailsSelectionBindings: Array<ShortcutBinding<CommitDetailsSelectionAction>> =
	[
		{
			id: "details-move-up",
			description: "up",
			keys: ["ArrowUp", "k"],
			action: { _tag: "Move", offset: -1 },
		},
		{
			id: "details-move-down",
			description: "down",
			keys: ["ArrowDown", "j"],
			action: { _tag: "Move", offset: 1 },
		},
		{
			id: "details-close",
			description: "close",
			keys: ["ArrowLeft", "Escape"],
			action: { _tag: "Close" },
			repeat: false,
		},
	];

import { type RefInfo } from "@gitbutler/but-sdk";
import { Match } from "effect";

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

export const getCommitDetailsSelectionAction = (
	event: KeyboardEvent,
): CommitDetailsSelectionAction | null =>
	Match.value(event.key).pipe(
		Match.whenOr("ArrowUp", "k", (): CommitDetailsSelectionAction | null => ({
			_tag: "Move",
			offset: -1,
		})),
		Match.whenOr("ArrowDown", "j", (): CommitDetailsSelectionAction | null => ({
			_tag: "Move",
			offset: 1,
		})),
		Match.whenOr("ArrowLeft", "Escape", (): CommitDetailsSelectionAction | null =>
			!event.repeat ? { _tag: "Close" } : null,
		),
		Match.orElse((): CommitDetailsSelectionAction | null => null),
	);
